use std::{
    collections::HashMap,
    future::Future,
    sync::{Mutex, MutexGuard, OnceLock},
    time::Duration,
};

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{http::StatusCode, Json};
use futures_util::TryStreamExt;
use id_core::{
    catalog::{
        AllocationStatus, CheckStatus, Checks, IdentifierKind, IdentifierPart, ReferenceData,
        ValidationReport,
    },
    identifiers::business::{
        lei::{gleif_record_api_url, validate_lei, LeiError},
        vat_id::{validate_german_vat_id, VIES_VALIDATION_URL},
    },
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ValidationApiResponses;
use crate::{parse_validate_payload, ApiError, ErrorResponse, ValidatePayload, ValidateRequest};

const BUSINESS_VALIDATION_WARNING: &str =
    "Offline validation does not prove current registry status, assignment, or entity identity.";
const GLEIF_JSON_API_MEDIA_TYPE: &str = "application/vnd.api+json";
const GLEIF_LOOKUP_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_GLEIF_RESPONSE_BYTES: usize = 512 * 1024;
const LEI_CACHE_MAX_ENTRIES: usize = 256;
const LEI_FOUND_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const LEI_NOT_FOUND_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const LEI_UPSTREAM_RATE_LIMIT: u32 = 60;
const LEI_UPSTREAM_RATE_WINDOW: Duration = Duration::from_secs(60);

fn invalid_report(
    kind: IdentifierKind,
    input: String,
    error: String,
    checksum: CheckStatus,
) -> ValidationReport {
    ValidationReport {
        kind,
        input,
        normalized: None,
        valid: false,
        checks: Checks {
            syntax: if checksum == CheckStatus::Invalid {
                CheckStatus::Valid
            } else {
                CheckStatus::Invalid
            },
            checksum,
            directory: CheckStatus::NotChecked,
            assignment: CheckStatus::Unknown,
        },
        allocation_status: AllocationStatus::Unknown,
        synthetic: None,
        production_usable: None,
        parts: Vec::new(),
        reference_data: None,
        warnings: Vec::new(),
        errors: vec![error],
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/business/tax/vat-id/validate",
    operation_id = "businessVatIdValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Unternehmen · Stammdaten & Register"
)]
pub(crate) async fn handle_vat_id_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_german_vat_id(&input) {
        Ok(validation) => ValidationReport {
            kind: IdentifierKind::VatId,
            input: input.clone(),
            normalized: Some(validation.parts.electronic),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                // No public German national checksum algorithm is available.
                checksum: CheckStatus::NotApplicable,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("country", validation.parts.country_code),
                IdentifierPart::new("national_identifier", validation.parts.national_identifier),
                IdentifierPart::new("checksum_status", "not_available"),
                IdentifierPart::new("lookup_status", "not_performed"),
            ],
            reference_data: None,
            warnings: vec![
                BUSINESS_VALIDATION_WARNING.to_string(),
                format!("Current validity requires a BZSt/VIES query: {VIES_VALIDATION_URL}"),
            ],
            errors: Vec::new(),
        },
        Err(error) => invalid_report(
            IdentifierKind::VatId,
            input,
            error.to_string(),
            CheckStatus::NotApplicable,
        ),
    };
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/business/organizations/lei/validate",
    operation_id = "businessLeiValidate",
    request_body = ValidateRequest,
    responses(ValidationApiResponses),
    tag = "Unternehmen · Stammdaten & Register"
)]
pub(crate) async fn handle_lei_validate(
    payload: ValidatePayload,
) -> Result<Json<ValidationReport>, ApiError> {
    let request = parse_validate_payload(payload)?;
    let input = request.id;
    let report = match validate_lei(&input) {
        Ok(validation) => ValidationReport {
            kind: IdentifierKind::Lei,
            input: input.clone(),
            normalized: Some(validation.parts.value),
            valid: true,
            checks: Checks {
                syntax: CheckStatus::Valid,
                checksum: CheckStatus::Valid,
                directory: CheckStatus::NotChecked,
                assignment: CheckStatus::Unknown,
            },
            allocation_status: AllocationStatus::Unknown,
            synthetic: None,
            production_usable: None,
            parts: vec![
                IdentifierPart::new("issuer_prefix", validation.parts.issuer_prefix),
                IdentifierPart::new("entity_specific", validation.parts.entity_specific),
                IdentifierPart::new("check_digits", validation.parts.check_digits),
            ],
            reference_data: Some(ReferenceData {
                name: "gleif_global_lei_index".to_string(),
                version: None,
                valid_from: None,
                valid_to: None,
                sha256: None,
            }),
            warnings: vec![BUSINESS_VALIDATION_WARNING.to_string()],
            errors: Vec::new(),
        },
        Err(error) => invalid_report(
            IdentifierKind::Lei,
            input,
            error.to_string(),
            if matches!(error, LeiError::ChecksumMismatch) {
                CheckStatus::Invalid
            } else {
                CheckStatus::NotChecked
            },
        ),
    };
    Ok(Json(report))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LeiLookupStatus {
    Found,
    NotFound,
    Unknown,
    UpstreamError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LeiLookupCacheStatus {
    /// Returned from the bounded in-process/isolate cache.
    Hit,
    /// Fetched from GLEIF and stored in the cache.
    Miss,
    /// Not cacheable, for example an upstream failure.
    NotStored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct LeiRegistryRecord {
    pub legal_name: Option<String>,
    pub jurisdiction: Option<String>,
    pub entity_status: Option<String>,
    pub registration_status: Option<String>,
    pub managing_lou: Option<String>,
    pub next_renewal_date: Option<String>,
    pub last_update_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub(crate) struct LeiLookupResponse {
    pub kind: IdentifierKind,
    pub value: String,
    pub registry: String,
    pub lookup_url: String,
    pub lookup_status: LeiLookupStatus,
    pub upstream_http_status: Option<u16>,
    pub registry_as_of: Option<String>,
    pub record: Option<LeiRegistryRecord>,
    pub checks: Checks,
    pub allocation_status: AllocationStatus,
    pub warnings: Vec<String>,
    pub upstream_error: Option<String>,
    pub cache_status: LeiLookupCacheStatus,
    /// Remaining cache lifetime for a hit, or the configured lifetime for a miss.
    pub cache_ttl_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GleifApiEnvelope {
    #[serde(default)]
    meta: Option<GleifMeta>,
    #[serde(default)]
    data: Option<GleifData>,
}

#[derive(Debug, Deserialize)]
struct GleifMeta {
    #[serde(rename = "goldenCopy")]
    golden_copy: Option<GleifGoldenCopy>,
}

#[derive(Debug, Deserialize)]
struct GleifGoldenCopy {
    #[serde(rename = "publishDate")]
    publish_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GleifData {
    id: String,
    attributes: GleifAttributes,
}

#[derive(Debug, Deserialize)]
struct GleifAttributes {
    lei: String,
    #[serde(default)]
    entity: Option<GleifEntity>,
    #[serde(default)]
    registration: Option<GleifRegistration>,
}

#[derive(Debug, Deserialize)]
struct GleifEntity {
    #[serde(rename = "legalName")]
    legal_name: Option<GleifLegalName>,
    jurisdiction: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GleifLegalName {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GleifRegistration {
    status: Option<String>,
    #[serde(rename = "managingLou")]
    managing_lou: Option<String>,
    #[serde(rename = "nextRenewalDate")]
    next_renewal_date: Option<String>,
    #[serde(rename = "lastUpdateDate")]
    last_update_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LookupHttpResponse {
    status: u16,
    body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LookupTransportError {
    message: String,
}

struct LookupOutcome {
    lookup_status: LeiLookupStatus,
    upstream_http_status: Option<u16>,
    registry_as_of: Option<String>,
    record: Option<LeiRegistryRecord>,
    checks: Checks,
    allocation_status: AllocationStatus,
    warnings: Vec<String>,
    upstream_error: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedLeiLookup {
    response: LeiLookupResponse,
    expires_at_millis: u64,
    last_access: u64,
}

#[derive(Debug)]
struct LeiLookupCache {
    entries: HashMap<String, CachedLeiLookup>,
    max_entries: usize,
    found_ttl: Duration,
    not_found_ttl: Duration,
    access_counter: u64,
}

impl LeiLookupCache {
    fn new(max_entries: usize, found_ttl: Duration, not_found_ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            found_ttl,
            not_found_ttl,
            access_counter: 0,
        }
    }

    fn get(&mut self, value: &str, now_millis: u64) -> Option<LeiLookupResponse> {
        self.remove_expired(now_millis);
        let entry = self.entries.get_mut(value)?;
        self.access_counter = self.access_counter.saturating_add(1);
        entry.last_access = self.access_counter;

        let mut response = entry.response.clone();
        response.cache_status = LeiLookupCacheStatus::Hit;
        response.cache_ttl_seconds = Some(millis_to_ceil_seconds(
            entry.expires_at_millis.saturating_sub(now_millis),
        ));
        Some(response)
    }

    fn store(
        &mut self,
        value: String,
        mut response: LeiLookupResponse,
        now_millis: u64,
    ) -> LeiLookupResponse {
        let ttl = match response.lookup_status {
            LeiLookupStatus::Found => self.found_ttl,
            LeiLookupStatus::NotFound => self.not_found_ttl,
            LeiLookupStatus::Unknown | LeiLookupStatus::UpstreamError => return response,
        };
        let ttl_millis = duration_millis(ttl);
        response.cache_status = LeiLookupCacheStatus::Miss;
        response.cache_ttl_seconds = Some(millis_to_ceil_seconds(ttl_millis));

        if self.max_entries == 0 {
            response.cache_status = LeiLookupCacheStatus::NotStored;
            response.cache_ttl_seconds = None;
            return response;
        }

        self.remove_expired(now_millis);
        if !self.entries.contains_key(&value) && self.entries.len() >= self.max_entries {
            self.evict_least_recently_used();
        }

        self.access_counter = self.access_counter.saturating_add(1);
        self.entries.insert(
            value,
            CachedLeiLookup {
                response: response.clone(),
                expires_at_millis: now_millis.saturating_add(ttl_millis),
                last_access: self.access_counter,
            },
        );
        response
    }

    fn remove_expired(&mut self, now_millis: u64) {
        self.entries
            .retain(|_, entry| entry.expires_at_millis > now_millis);
    }

    fn evict_least_recently_used(&mut self) {
        let key = self
            .entries
            .iter()
            .min_by(|(left_key, left), (right_key, right)| {
                left.last_access
                    .cmp(&right.last_access)
                    .then_with(|| left_key.cmp(right_key))
            })
            .map(|(key, _)| key.clone());
        if let Some(key) = key {
            self.entries.remove(&key);
        }
    }
}

#[derive(Debug)]
struct FixedWindowRateLimiter {
    limit: u32,
    window_millis: u64,
    window_started_at_millis: u64,
    used: u32,
}

impl FixedWindowRateLimiter {
    fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window_millis: duration_millis(window),
            window_started_at_millis: 0,
            used: 0,
        }
    }

    fn check(&mut self, now_millis: u64) -> Result<(), u64> {
        let window_ends_at = self
            .window_started_at_millis
            .saturating_add(self.window_millis);
        if now_millis < self.window_started_at_millis || now_millis >= window_ends_at {
            self.window_started_at_millis = now_millis;
            self.used = 0;
        }

        if self.used >= self.limit {
            return Err(millis_to_ceil_seconds(
                self.window_started_at_millis
                    .saturating_add(self.window_millis)
                    .saturating_sub(now_millis),
            ));
        }
        self.used = self.used.saturating_add(1);
        Ok(())
    }
}

#[derive(Debug)]
struct LeiLookupRuntime {
    cache: LeiLookupCache,
    upstream_limiter: FixedWindowRateLimiter,
}

impl LeiLookupRuntime {
    fn production() -> Self {
        Self {
            cache: LeiLookupCache::new(
                LEI_CACHE_MAX_ENTRIES,
                LEI_FOUND_CACHE_TTL,
                LEI_NOT_FOUND_CACHE_TTL,
            ),
            upstream_limiter: FixedWindowRateLimiter::new(
                LEI_UPSTREAM_RATE_LIMIT,
                LEI_UPSTREAM_RATE_WINDOW,
            ),
        }
    }
}

static LEI_LOOKUP_RUNTIME: OnceLock<Mutex<LeiLookupRuntime>> = OnceLock::new();

fn lookup_runtime() -> &'static Mutex<LeiLookupRuntime> {
    LEI_LOOKUP_RUNTIME.get_or_init(|| Mutex::new(LeiLookupRuntime::production()))
}

fn lock_runtime(runtime: &Mutex<LeiLookupRuntime>) -> MutexGuard<'_, LeiLookupRuntime> {
    runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn millis_to_ceil_seconds(millis: u64) -> u64 {
    millis.saturating_add(999) / 1_000
}

#[cfg(not(target_arch = "wasm32"))]
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(duration_millis)
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
fn now_millis() -> u64 {
    worker::Date::now().as_millis()
}

impl LookupTransportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

fn lookup_checks(directory: CheckStatus, assignment: CheckStatus) -> Checks {
    Checks {
        syntax: CheckStatus::Valid,
        checksum: CheckStatus::Valid,
        directory,
        assignment,
    }
}

fn lookup_response(value: &str, lookup_url: &str, outcome: LookupOutcome) -> LeiLookupResponse {
    LeiLookupResponse {
        kind: IdentifierKind::Lei,
        value: value.to_string(),
        registry: "GLEIF Global LEI Index".to_string(),
        lookup_url: lookup_url.to_string(),
        lookup_status: outcome.lookup_status,
        upstream_http_status: outcome.upstream_http_status,
        registry_as_of: outcome.registry_as_of,
        record: outcome.record,
        checks: outcome.checks,
        allocation_status: outcome.allocation_status,
        warnings: outcome.warnings,
        upstream_error: outcome.upstream_error,
        cache_status: LeiLookupCacheStatus::NotStored,
        cache_ttl_seconds: None,
    }
}

fn parse_gleif_response(
    value: &str,
    lookup_url: &str,
    response: LookupHttpResponse,
) -> LeiLookupResponse {
    match response.status {
        200 => {
            let envelope = match serde_json::from_slice::<GleifApiEnvelope>(&response.body) {
                Ok(envelope) => envelope,
                Err(error) => {
                    return lookup_response(
                        value,
                        lookup_url,
                        LookupOutcome {
                            lookup_status: LeiLookupStatus::UpstreamError,
                            upstream_http_status: Some(response.status),
                            registry_as_of: None,
                            record: None,
                            checks: lookup_checks(CheckStatus::Unknown, CheckStatus::Unknown),
                            allocation_status: AllocationStatus::Unknown,
                            warnings: vec![
                                "GLEIF returned an unreadable registry response.".to_string(),
                            ],
                            upstream_error: Some(format!("Invalid GLEIF JSON: {error}")),
                        },
                    );
                }
            };
            let registry_as_of = envelope
                .meta
                .and_then(|meta| meta.golden_copy)
                .and_then(|golden_copy| golden_copy.publish_date);
            let Some(data) = envelope.data else {
                return lookup_response(
                    value,
                    lookup_url,
                    LookupOutcome {
                        lookup_status: LeiLookupStatus::Unknown,
                        upstream_http_status: Some(response.status),
                        registry_as_of,
                        record: None,
                        checks: lookup_checks(CheckStatus::Unknown, CheckStatus::Unknown),
                        allocation_status: AllocationStatus::Unknown,
                        warnings: vec![
                            "GLEIF returned a successful response without a record; assignment remains unknown."
                                .to_string(),
                        ],
                        upstream_error: None,
                    },
                );
            };
            if data.id != value || data.attributes.lei != value {
                return lookup_response(
                    value,
                    lookup_url,
                    LookupOutcome {
                        lookup_status: LeiLookupStatus::UpstreamError,
                        upstream_http_status: Some(response.status),
                        registry_as_of,
                        record: None,
                        checks: lookup_checks(CheckStatus::Unknown, CheckStatus::Unknown),
                        allocation_status: AllocationStatus::Unknown,
                        warnings: vec![
                            "GLEIF returned a record for a different LEI.".to_string(),
                        ],
                        upstream_error: Some(
                            "GLEIF response identifier did not match the requested LEI".to_string(),
                        ),
                    },
                );
            }

            let entity = data.attributes.entity;
            let registration = data.attributes.registration;
            let record = LeiRegistryRecord {
                legal_name: entity
                    .as_ref()
                    .and_then(|entity| entity.legal_name.as_ref())
                    .and_then(|name| name.name.clone()),
                jurisdiction: entity
                    .as_ref()
                    .and_then(|entity| entity.jurisdiction.clone()),
                entity_status: entity.as_ref().and_then(|entity| entity.status.clone()),
                registration_status: registration
                    .as_ref()
                    .and_then(|registration| registration.status.clone()),
                managing_lou: registration
                    .as_ref()
                    .and_then(|registration| registration.managing_lou.clone()),
                next_renewal_date: registration
                    .as_ref()
                    .and_then(|registration| registration.next_renewal_date.clone()),
                last_update_date: registration
                    .as_ref()
                    .and_then(|registration| registration.last_update_date.clone()),
            };
            lookup_response(
                value,
                lookup_url,
                LookupOutcome {
                    lookup_status: LeiLookupStatus::Found,
                    upstream_http_status: Some(response.status),
                    registry_as_of,
                    record: Some(record),
                    checks: lookup_checks(CheckStatus::Found, CheckStatus::Found),
                    allocation_status: AllocationStatus::Allocated,
                    warnings: vec![
                        "The LEI was found in GLEIF; registry status does not independently verify the entity's identity or suitability for a transaction."
                            .to_string(),
                    ],
                    upstream_error: None,
                },
            )
        }
        404 => lookup_response(
            value,
            lookup_url,
            LookupOutcome {
                lookup_status: LeiLookupStatus::NotFound,
                upstream_http_status: Some(response.status),
                registry_as_of: None,
                record: None,
                checks: lookup_checks(CheckStatus::NotFound, CheckStatus::Unknown),
                allocation_status: AllocationStatus::Unknown,
                warnings: vec![
                    "No GLEIF record was found at lookup time; this is not permanent proof that the identifier was never allocated."
                        .to_string(),
                ],
                upstream_error: None,
            },
        ),
        status => lookup_response(
            value,
            lookup_url,
            LookupOutcome {
                lookup_status: LeiLookupStatus::UpstreamError,
                upstream_http_status: Some(status),
                registry_as_of: None,
                record: None,
                checks: lookup_checks(CheckStatus::Unknown, CheckStatus::Unknown),
                allocation_status: AllocationStatus::Unknown,
                warnings: vec!["GLEIF could not provide usable registry evidence.".to_string()],
                upstream_error: Some(format!("GLEIF returned HTTP {status}")),
            },
        ),
    }
}

fn transport_error_response(
    value: &str,
    lookup_url: &str,
    error: LookupTransportError,
) -> LeiLookupResponse {
    lookup_response(
        value,
        lookup_url,
        LookupOutcome {
            lookup_status: LeiLookupStatus::UpstreamError,
            upstream_http_status: None,
            registry_as_of: None,
            record: None,
            checks: lookup_checks(CheckStatus::Unknown, CheckStatus::Unknown),
            allocation_status: AllocationStatus::Unknown,
            warnings: vec!["GLEIF could not be reached; assignment remains unknown.".to_string()],
            upstream_error: Some(error.message),
        },
    )
}

fn public_status(response: &LeiLookupResponse) -> StatusCode {
    if response.lookup_status == LeiLookupStatus::UpstreamError {
        StatusCode::BAD_GATEWAY
    } else {
        StatusCode::OK
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_gleif_response(url: String) -> Result<LookupHttpResponse, LookupTransportError> {
    let response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, GLEIF_JSON_API_MEDIA_TYPE)
        .timeout(GLEIF_LOOKUP_TIMEOUT)
        .send()
        .await
        .map_err(|error| LookupTransportError::new(format!("GLEIF request failed: {error}")))?;
    let status = response.status().as_u16();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_GLEIF_RESPONSE_BYTES as u64)
    {
        return Err(LookupTransportError::new(
            "GLEIF response exceeded the configured size limit",
        ));
    }

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream
        .try_next()
        .await
        .map_err(|error| LookupTransportError::new(format!("GLEIF response failed: {error}")))?
    {
        if body.len().saturating_add(chunk.len()) > MAX_GLEIF_RESPONSE_BYTES {
            return Err(LookupTransportError::new(
                "GLEIF response exceeded the configured size limit",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(LookupHttpResponse { status, body })
}

#[cfg(target_arch = "wasm32")]
async fn fetch_gleif_response_inner(
    url: String,
) -> Result<LookupHttpResponse, LookupTransportError> {
    let headers = worker::Headers::new();
    headers
        .set("accept", GLEIF_JSON_API_MEDIA_TYPE)
        .map_err(|error| LookupTransportError::new(format!("Invalid GLEIF request: {error}")))?;
    let mut init = worker::RequestInit::new();
    init.with_headers(headers);
    let request = worker::Request::new_with_init(&url, &init)
        .map_err(|error| LookupTransportError::new(format!("Invalid GLEIF request: {error}")))?;
    let fetch = worker::Fetch::Request(request);
    let timeout_millis = u32::try_from(GLEIF_LOOKUP_TIMEOUT.as_millis()).unwrap_or(u32::MAX);
    let signal = worker::AbortSignal::from(web_sys::AbortSignal::timeout_with_u32(timeout_millis));
    let mut response = fetch
        .send_with_signal(&signal)
        .await
        .map_err(|error| LookupTransportError::new(format!("GLEIF request failed: {error}")))?;
    let status = response.status_code();
    if response
        .headers()
        .get("content-length")
        .map_err(|error| LookupTransportError::new(format!("Invalid GLEIF response: {error}")))?
        .and_then(|length| length.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_GLEIF_RESPONSE_BYTES)
    {
        return Err(LookupTransportError::new(
            "GLEIF response exceeded the configured size limit",
        ));
    }

    let mut body = Vec::new();
    let mut stream = response
        .stream()
        .map_err(|error| LookupTransportError::new(format!("Invalid GLEIF response: {error}")))?;
    while let Some(mut chunk) = stream
        .try_next()
        .await
        .map_err(|error| LookupTransportError::new(format!("GLEIF response failed: {error}")))?
    {
        if body.len().saturating_add(chunk.len()) > MAX_GLEIF_RESPONSE_BYTES {
            return Err(LookupTransportError::new(
                "GLEIF response exceeded the configured size limit",
            ));
        }
        body.append(&mut chunk);
    }
    Ok(LookupHttpResponse { status, body })
}

#[cfg(target_arch = "wasm32")]
async fn fetch_gleif_response(url: String) -> Result<LookupHttpResponse, LookupTransportError> {
    // The Workers Fetch API owns JavaScript values whose futures are `!Send`.
    // Axum requires handler futures to be `Send`; workers-rs provides this
    // adapter specifically because a Worker isolate executes on one thread.
    worker::send::SendFuture::new(fetch_gleif_response_inner(url)).await
}

async fn perform_lei_lookup<F, Fut>(
    value: String,
    lookup_url: String,
    fetch: F,
) -> (StatusCode, LeiLookupResponse)
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<LookupHttpResponse, LookupTransportError>>,
{
    let response = match fetch(lookup_url.clone()).await {
        Ok(response) => parse_gleif_response(&value, &lookup_url, response),
        Err(error) => transport_error_response(&value, &lookup_url, error),
    };
    (public_status(&response), response)
}

async fn perform_cached_lei_lookup<F, Fut>(
    runtime: &Mutex<LeiLookupRuntime>,
    value: String,
    lookup_url: String,
    now_millis: u64,
    fetch: F,
) -> Result<(StatusCode, LeiLookupResponse), u64>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<LookupHttpResponse, LookupTransportError>>,
{
    {
        let mut runtime = lock_runtime(runtime);
        if let Some(response) = runtime.cache.get(&value, now_millis) {
            return Ok((StatusCode::OK, response));
        }
        runtime.upstream_limiter.check(now_millis)?;
    }

    let (status, response) = perform_lei_lookup(value.clone(), lookup_url, fetch).await;
    let response = lock_runtime(runtime)
        .cache
        .store(value, response, now_millis);
    Ok((status, response))
}

#[allow(dead_code)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum LeiLookupApiResponses {
    /// Current lookup result from the public GLEIF JSON:API.
    #[response(status = 200)]
    Success(LeiLookupResponse),
    /// GLEIF was unavailable or returned an unusable response.
    #[response(status = 502)]
    UpstreamFailure(LeiLookupResponse),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// The request schema or LEI is invalid.
    #[response(status = 422)]
    InvalidRequest(ErrorResponse),
    /// The application-side upstream budget or the Cloudflare edge limit was exhausted.
    #[response(status = 429)]
    RateLimited(ErrorResponse),
}

#[utoipa::path(
    post,
    path = "/api/v1/business/organizations/lei/lookup",
    operation_id = "businessLeiLookup",
    request_body = ValidateRequest,
    responses(LeiLookupApiResponses),
    tag = "Unternehmen · Stammdaten & Register"
)]
pub(crate) async fn handle_lei_lookup(
    payload: ValidatePayload,
) -> Result<(StatusCode, Json<LeiLookupResponse>), ApiError> {
    let request = parse_validate_payload(payload)?;
    let lookup_url = gleif_record_api_url(&request.id)
        .map_err(|error| ApiError::invalid_request(error.to_string()))?;
    let validation =
        validate_lei(&request.id).map_err(|error| ApiError::invalid_request(error.to_string()))?;

    let (status, response) = perform_cached_lei_lookup(
        lookup_runtime(),
        validation.parts.value,
        lookup_url,
        now_millis(),
        fetch_gleif_response,
    )
    .await
    .map_err(crate::ApiError::rate_limited)?;
    Ok((status, Json(response)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_runtime(
        max_entries: usize,
        found_ttl: Duration,
        not_found_ttl: Duration,
        limit: u32,
        window: Duration,
    ) -> Mutex<LeiLookupRuntime> {
        Mutex::new(LeiLookupRuntime {
            cache: LeiLookupCache::new(max_entries, found_ttl, not_found_ttl),
            upstream_limiter: FixedWindowRateLimiter::new(limit, window),
        })
    }

    fn payload(id: &str) -> ValidatePayload {
        Ok(Json(ValidateRequest { id: id.to_string() }))
    }

    #[tokio::test]
    async fn vat_validation_never_claims_checksum_or_assignment_evidence() {
        let Json(report) = handle_vat_id_validate(payload("de 123 456 789"))
            .await
            .unwrap();
        assert!(report.valid);
        assert_eq!(report.normalized.as_deref(), Some("DE123456789"));
        assert_eq!(report.checks.checksum, CheckStatus::NotApplicable);
        assert_eq!(report.checks.directory, CheckStatus::NotChecked);
        assert_eq!(report.allocation_status, AllocationStatus::Unknown);
    }

    #[tokio::test]
    async fn lei_validation_separates_checksum_from_registry_evidence() {
        let Json(report) = handle_lei_validate(payload("506700GE1G29325QX363"))
            .await
            .unwrap();
        assert!(report.valid);
        assert_eq!(report.checks.checksum, CheckStatus::Valid);
        assert_eq!(report.checks.directory, CheckStatus::NotChecked);
        assert_eq!(report.checks.assignment, CheckStatus::Unknown);
    }

    fn lookup_url() -> String {
        gleif_record_api_url("506700GE1G29325QX363").unwrap()
    }

    #[tokio::test]
    async fn injected_gleif_record_is_reported_as_found_without_a_live_request() {
        let body = br#"{
            "meta":{"goldenCopy":{"publishDate":"2026-08-14T00:00:00Z"}},
            "data":{
                "type":"lei-records",
                "id":"506700GE1G29325QX363",
                "attributes":{
                    "lei":"506700GE1G29325QX363",
                    "entity":{
                        "legalName":{"name":"Global Legal Entity Identifier Foundation"},
                        "jurisdiction":"CH",
                        "status":"ACTIVE"
                    },
                    "registration":{
                        "status":"ISSUED",
                        "managingLou":"506700LOLO7M6V0E4247",
                        "nextRenewalDate":"2027-03-15T00:00:00Z",
                        "lastUpdateDate":"2026-04-23T05:00:06Z"
                    }
                }
            }
        }"#
        .to_vec();
        let (status, response) = perform_lei_lookup(
            "506700GE1G29325QX363".to_string(),
            lookup_url(),
            |_| async move { Ok(LookupHttpResponse { status: 200, body }) },
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.lookup_status, LeiLookupStatus::Found);
        assert_eq!(response.checks.directory, CheckStatus::Found);
        assert_eq!(response.checks.assignment, CheckStatus::Found);
        assert_eq!(response.allocation_status, AllocationStatus::Allocated);
        assert_eq!(
            response.registry_as_of.as_deref(),
            Some("2026-08-14T00:00:00Z")
        );
        assert_eq!(
            response
                .record
                .as_ref()
                .and_then(|record| record.legal_name.as_deref()),
            Some("Global Legal Entity Identifier Foundation")
        );
    }

    #[tokio::test]
    async fn injected_404_is_not_found_but_not_claimed_as_unallocated() {
        let (status, response) = perform_lei_lookup(
            "506700GE1G29325QX363".to_string(),
            lookup_url(),
            |_| async {
                Ok(LookupHttpResponse {
                    status: 404,
                    body: br#"{"errors":[{"status":"404","title":"Not Found"}]}"#.to_vec(),
                })
            },
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.lookup_status, LeiLookupStatus::NotFound);
        assert_eq!(response.checks.directory, CheckStatus::NotFound);
        assert_eq!(response.checks.assignment, CheckStatus::Unknown);
        assert_eq!(response.allocation_status, AllocationStatus::Unknown);
        assert!(response.record.is_none());
    }

    #[tokio::test]
    async fn found_and_not_found_results_use_distinct_bounded_ttls() {
        let runtime = test_runtime(
            4,
            Duration::from_secs(15),
            Duration::from_secs(5),
            10,
            Duration::from_secs(60),
        );
        let value = "506700GE1G29325QX363".to_string();
        let body = br#"{
            "data":{
                "id":"506700GE1G29325QX363",
                "attributes":{"lei":"506700GE1G29325QX363"}
            }
        }"#
        .to_vec();

        let (_, first) = perform_cached_lei_lookup(
            &runtime,
            value.clone(),
            lookup_url(),
            1_000,
            |_| async move { Ok(LookupHttpResponse { status: 200, body }) },
        )
        .await
        .unwrap();
        assert_eq!(first.cache_status, LeiLookupCacheStatus::Miss);
        assert_eq!(first.cache_ttl_seconds, Some(15));

        let (_, cached) =
            perform_cached_lei_lookup(&runtime, value.clone(), lookup_url(), 6_001, |_| async {
                panic!("a fresh GLEIF request was made for a positive cache hit")
            })
            .await
            .unwrap();
        assert_eq!(cached.cache_status, LeiLookupCacheStatus::Hit);
        assert_eq!(cached.cache_ttl_seconds, Some(10));

        let negative_value = "negative-fixture".to_string();
        let (_, negative) = perform_cached_lei_lookup(
            &runtime,
            negative_value.clone(),
            "https://example.invalid/negative-fixture".to_string(),
            7_000,
            |_| async {
                Ok(LookupHttpResponse {
                    status: 404,
                    body: Vec::new(),
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(negative.cache_status, LeiLookupCacheStatus::Miss);
        assert_eq!(negative.cache_ttl_seconds, Some(5));

        let (_, refreshed) = perform_cached_lei_lookup(
            &runtime,
            negative_value,
            "https://example.invalid/negative-fixture".to_string(),
            12_000,
            |_| async {
                Ok(LookupHttpResponse {
                    status: 404,
                    body: Vec::new(),
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(refreshed.cache_status, LeiLookupCacheStatus::Miss);
    }

    #[test]
    fn cache_evicts_the_least_recently_used_entry_at_its_hard_limit() {
        let mut cache = LeiLookupCache::new(2, Duration::from_secs(60), Duration::from_secs(60));
        let response_for = |value: &str| {
            parse_gleif_response(
                value,
                "https://example.invalid",
                LookupHttpResponse {
                    status: 404,
                    body: Vec::new(),
                },
            )
        };

        cache.store("A".to_string(), response_for("A"), 0);
        cache.store("B".to_string(), response_for("B"), 0);
        assert!(cache.get("A", 1).is_some());
        cache.store("C".to_string(), response_for("C"), 2);

        assert_eq!(cache.entries.len(), 2);
        assert!(cache.get("A", 3).is_some());
        assert!(cache.get("B", 3).is_none());
        assert!(cache.get("C", 3).is_some());
    }

    #[tokio::test]
    async fn upstream_failures_are_not_cached_and_cached_hits_bypass_the_upstream_budget() {
        let runtime = test_runtime(
            4,
            Duration::from_secs(60),
            Duration::from_secs(60),
            1,
            Duration::from_secs(60),
        );
        let first = perform_cached_lei_lookup(
            &runtime,
            "upstream-error".to_string(),
            "https://example.invalid/error".to_string(),
            1,
            |_| async { Err(LookupTransportError::new("simulated timeout")) },
        )
        .await
        .unwrap();
        assert_eq!(first.1.cache_status, LeiLookupCacheStatus::NotStored);
        assert!(lock_runtime(&runtime).cache.entries.is_empty());

        // A new uncached value is rejected because the failed upstream attempt
        // consumed the single request budget for this window.
        let retry_after = perform_cached_lei_lookup(
            &runtime,
            "another-value".to_string(),
            "https://example.invalid/another".to_string(),
            2,
            |_| async {
                Ok(LookupHttpResponse {
                    status: 404,
                    body: Vec::new(),
                })
            },
        )
        .await
        .unwrap_err();
        assert_eq!(retry_after, 60);

        // Seed a cache entry directly and prove it is still served while the
        // upstream budget is exhausted.
        lock_runtime(&runtime).cache.store(
            "cached-value".to_string(),
            parse_gleif_response(
                "cached-value",
                "https://example.invalid/cached",
                LookupHttpResponse {
                    status: 404,
                    body: Vec::new(),
                },
            ),
            2,
        );
        let (_, cached) = perform_cached_lei_lookup(
            &runtime,
            "cached-value".to_string(),
            "https://example.invalid/cached".to_string(),
            3,
            |_| async { panic!("cache hits must not consume the upstream request budget") },
        )
        .await
        .unwrap();
        assert_eq!(cached.cache_status, LeiLookupCacheStatus::Hit);
    }

    #[tokio::test]
    async fn injected_transport_and_upstream_failures_return_bad_gateway() {
        let (status, response) = perform_lei_lookup(
            "506700GE1G29325QX363".to_string(),
            lookup_url(),
            |_| async { Err(LookupTransportError::new("simulated timeout")) },
        )
        .await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(response.lookup_status, LeiLookupStatus::UpstreamError);
        assert_eq!(response.checks.directory, CheckStatus::Unknown);
        assert_eq!(response.allocation_status, AllocationStatus::Unknown);

        let response = parse_gleif_response(
            "506700GE1G29325QX363",
            &lookup_url(),
            LookupHttpResponse {
                status: 503,
                body: Vec::new(),
            },
        );
        assert_eq!(response.lookup_status, LeiLookupStatus::UpstreamError);
        assert_eq!(response.upstream_http_status, Some(503));
    }

    #[tokio::test]
    async fn invalid_lei_is_rejected_before_any_network_lookup() {
        assert!(handle_lei_lookup(payload("506700GE1G29325QX364"))
            .await
            .is_err());
    }
}
