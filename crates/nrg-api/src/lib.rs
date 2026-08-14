//! Shared HTTP API used by both the native container server and Cloudflare Worker.

use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit},
    http::{header::CONTENT_TYPE, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, MethodRouter},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use utoipa::{OpenApi, ToSchema};

use id_core::{
    catalog::{
        AccountExistenceStatus, AllocationStatus, ApiMethod, ApiOperationDescriptor, CheckStatus,
        Checks, CollisionGuarantee, GenerateRequest, GeneratedIdentifier, GenerationProfile,
        IdentifierFormat, IdentifierKind, IdentifierPart, ReferenceData, Sector, ValidationReport,
    },
    generate_malo, generate_melo, generate_nelo, validate_malo, validate_melo, validate_nelo,
};

use catalog_api::{
    CatalogExample, CatalogIdentifier, CatalogOperation, CatalogResponse, CatalogSource,
};
use routes::{
    payments::{BicGenerateRequest, RfGenerateRequest},
    GenerateResponse,
};

mod catalog_api;
mod routes;

pub const OPENAPI_JSON_PATH: &str = "/api-docs/openapi.json";
pub const SWAGGER_UI_PATH: &str = "/swagger-ui";
const MAX_REQUEST_BODY_BYTES: usize = 1024;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ValidateRequest {
    /// The ID to validate.
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct GenerateMaloResponse {
    /// Generated MaLo-ID (11 digits).
    pub id: String,
    /// Check digit (last digit).
    pub checksum: u8,
    /// Issuing authority: DVGW or BDEW.
    pub issuer: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ValidateMaloResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct GenerateMeloResponse {
    /// Generated MeLo-ID (33 characters).
    pub id: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ValidateMeloResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct GenerateNeloResponse {
    /// Generated NeLo-ID (11 characters).
    pub id: String,
    /// Check digit (last character).
    pub checksum: u8,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ValidateNeloResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

pub(crate) type ValidatePayload = Result<Json<ValidateRequest>, JsonRejection>;

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
    retry_after_seconds: Option<u64>,
}

impl ApiError {
    fn invalid_json(rejection: JsonRejection) -> Self {
        let status = rejection.status();
        let message = match status {
            StatusCode::BAD_REQUEST => "Malformed JSON request body".to_string(),
            StatusCode::PAYLOAD_TOO_LARGE => {
                format!("Request body exceeds {MAX_REQUEST_BODY_BYTES} bytes")
            }
            StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                "Content-Type must be application/json".to_string()
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                "JSON request body must contain a string field named 'id'".to_string()
            }
            _ => "Invalid JSON request body".to_string(),
        };

        Self {
            status,
            message,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn invalid_generate_json(rejection: JsonRejection) -> Self {
        let status = rejection.status();
        let message = match status {
            StatusCode::BAD_REQUEST => "Malformed JSON request body".to_string(),
            StatusCode::PAYLOAD_TOO_LARGE => {
                format!("Request body exceeds {MAX_REQUEST_BODY_BYTES} bytes")
            }
            StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                "Content-Type must be application/json".to_string()
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                "JSON request body does not match the generation schema".to_string()
            }
            _ => "Invalid JSON request body".to_string(),
        };
        Self {
            status,
            message,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn invalid_request(message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message,
            retry_after_seconds: None,
        }
    }

    fn generation_failed(identifier: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to generate a valid {identifier}"),
            retry_after_seconds: None,
        }
    }

    pub(crate) fn generation_failed_with_message(identifier: &str, detail: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to generate a valid {identifier}: {detail}"),
            retry_after_seconds: None,
        }
    }

    pub(crate) fn rate_limited(retry_after_seconds: u64) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "LEI lookup rate limit exceeded; retry later".to_string(),
            retry_after_seconds: Some(retry_after_seconds.max(1)),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorResponse {
                error: self.message,
                retry_after_seconds: self.retry_after_seconds,
            }),
        )
            .into_response();
        if let Some(seconds) = self.retry_after_seconds {
            if let Ok(value) = seconds.to_string().parse() {
                response
                    .headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, value);
            }
        }
        response
    }
}

/// Builds the standard LEI lookup abuse-protection response for deployment
/// adapters that enforce an additional edge-side limit.
pub fn lei_lookup_rate_limit_response(retry_after_seconds: u64) -> Response {
    ApiError::rate_limited(retry_after_seconds).into_response()
}

pub(crate) fn parse_validate_payload(
    payload: ValidatePayload,
) -> Result<ValidateRequest, ApiError> {
    payload
        .map(|Json(request)| request)
        .map_err(ApiError::invalid_json)
}

#[utoipa::path(
    get,
    path = "/api/malo/generate",
    responses(
        (status = 200, description = "Generated MaLo-ID", body = GenerateMaloResponse),
        (status = 500, description = "Generated ID failed an internal invariant", body = ErrorResponse)
    ),
    tag = "MaLo-ID"
)]
async fn handle_malo_generate() -> Response {
    let id = generate_malo();
    match validate_malo(&id) {
        Ok(info) => Json(GenerateMaloResponse {
            id: info.id,
            checksum: info.checksum,
            issuer: info.issuer.to_string(),
        })
        .into_response(),
        Err(_) => ApiError::generation_failed("MaLo-ID").into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/malo/validate",
    request_body = ValidateRequest,
    responses(
        (status = 200, description = "Validation result", body = ValidateMaloResponse),
        (status = 400, description = "Malformed JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Content-Type is not application/json", body = ErrorResponse),
        (status = 422, description = "JSON does not match the request schema", body = ErrorResponse)
    ),
    tag = "MaLo-ID"
)]
async fn handle_malo_validate(payload: ValidatePayload) -> Response {
    let request = match parse_validate_payload(payload) {
        Ok(request) => request,
        Err(error) => return error.into_response(),
    };

    Json(match validate_malo(&request.id) {
        Ok(info) => ValidateMaloResponse {
            valid: true,
            checksum: Some(info.checksum),
            issuer: Some(info.issuer.to_string()),
            error: None,
        },
        Err(error) => ValidateMaloResponse {
            valid: false,
            checksum: None,
            issuer: None,
            error: Some(error),
        },
    })
    .into_response()
}

#[utoipa::path(
    get,
    path = "/api/melo/generate",
    responses(
        (status = 200, description = "Generated MeLo-ID", body = GenerateMeloResponse),
        (status = 500, description = "Generated ID failed an internal invariant", body = ErrorResponse)
    ),
    tag = "MeLo-ID"
)]
async fn handle_melo_generate() -> Response {
    let id = generate_melo();
    match validate_melo(&id) {
        Ok(()) => Json(GenerateMeloResponse { id }).into_response(),
        Err(_) => ApiError::generation_failed("MeLo-ID").into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/melo/validate",
    request_body = ValidateRequest,
    responses(
        (status = 200, description = "Validation result", body = ValidateMeloResponse),
        (status = 400, description = "Malformed JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Content-Type is not application/json", body = ErrorResponse),
        (status = 422, description = "JSON does not match the request schema", body = ErrorResponse)
    ),
    tag = "MeLo-ID"
)]
async fn handle_melo_validate(payload: ValidatePayload) -> Response {
    let request = match parse_validate_payload(payload) {
        Ok(request) => request,
        Err(error) => return error.into_response(),
    };

    Json(match validate_melo(&request.id) {
        Ok(()) => ValidateMeloResponse {
            valid: true,
            error: None,
        },
        Err(error) => ValidateMeloResponse {
            valid: false,
            error: Some(error),
        },
    })
    .into_response()
}

#[utoipa::path(
    get,
    path = "/api/nelo/generate",
    responses(
        (status = 200, description = "Generated NeLo-ID", body = GenerateNeloResponse),
        (status = 500, description = "Generated ID failed an internal invariant", body = ErrorResponse)
    ),
    tag = "NeLo-ID"
)]
async fn handle_nelo_generate() -> Response {
    let id = generate_nelo();
    match id
        .chars()
        .last()
        .and_then(|character| character.to_digit(10))
    {
        Some(checksum) if validate_nelo(&id).is_ok() => Json(GenerateNeloResponse {
            id,
            checksum: checksum as u8,
        })
        .into_response(),
        _ => ApiError::generation_failed("NeLo-ID").into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/nelo/validate",
    request_body = ValidateRequest,
    responses(
        (status = 200, description = "Validation result", body = ValidateNeloResponse),
        (status = 400, description = "Malformed JSON", body = ErrorResponse),
        (status = 413, description = "Request body too large", body = ErrorResponse),
        (status = 415, description = "Content-Type is not application/json", body = ErrorResponse),
        (status = 422, description = "JSON does not match the request schema", body = ErrorResponse)
    ),
    tag = "NeLo-ID"
)]
async fn handle_nelo_validate(payload: ValidatePayload) -> Response {
    let request = match parse_validate_payload(payload) {
        Ok(request) => request,
        Err(error) => return error.into_response(),
    };

    Json(match validate_nelo(&request.id) {
        Ok(()) => ValidateNeloResponse {
            valid: true,
            error: None,
        },
        Err(error) => ValidateNeloResponse {
            valid: false,
            error: Some(error),
        },
    })
    .into_response()
}

/// The only HTTP-handler declaration list.
///
/// Operation identifiers and expected methods are deliberately repeated here
/// only as compile-time bindings between catalog operations and concrete Rust
/// handlers. Paths and the effective router methods always come from the
/// central identifier/service catalog and are checked bidirectionally below.
macro_rules! public_operation_handlers {
    ($consumer:ident) => {
        $consumer!(
            ("legacyMaloGenerate", Get, handle_malo_generate),
            ("legacyMaloValidate", Post, handle_malo_validate),
            ("legacyMeloGenerate", Get, handle_melo_generate),
            ("legacyMeloValidate", Post, handle_melo_validate),
            ("legacyNeloGenerate", Get, handle_nelo_generate),
            ("legacyNeloValidate", Post, handle_nelo_validate),
            (
                "energyMaloGenerate",
                Post,
                routes::energy::handle_malo_generate
            ),
            (
                "energyMaloValidate",
                Post,
                routes::energy::handle_malo_validate
            ),
            (
                "energyMeloGenerate",
                Post,
                routes::energy::handle_melo_generate
            ),
            (
                "energyMeloValidate",
                Post,
                routes::energy::handle_melo_validate
            ),
            (
                "energyNeloGenerate",
                Post,
                routes::energy::handle_nelo_generate
            ),
            (
                "energyNeloValidate",
                Post,
                routes::energy::handle_nelo_validate
            ),
            (
                "energyNebeGenerate",
                Post,
                routes::energy::handle_nebe_generate
            ),
            (
                "energyNebeValidate",
                Post,
                routes::energy::handle_nebe_validate
            ),
            (
                "energyMarketPartnerIdGenerate",
                Post,
                routes::energy::handle_market_partner_generate
            ),
            (
                "energyMarketPartnerIdValidate",
                Post,
                routes::energy::handle_market_partner_validate
            ),
            (
                "energyClusterResourceIdGenerate",
                Post,
                routes::energy::handle_cr_generate
            ),
            (
                "energySteeringGroupIdGenerate",
                Post,
                routes::energy::handle_sg_generate
            ),
            (
                "energyControllableResourceIdGenerate",
                Post,
                routes::energy::handle_sr_generate
            ),
            (
                "energyTechnicalResourceIdGenerate",
                Post,
                routes::energy::handle_tr_generate
            ),
            (
                "energyPackageIdGenerate",
                Post,
                routes::energy::handle_package_generate
            ),
            (
                "paymentsIbanGenerate",
                Post,
                routes::payments::handle_iban_generate
            ),
            (
                "paymentsIbanValidate",
                Post,
                routes::payments::handle_iban_validate
            ),
            (
                "paymentsBicGenerate",
                Post,
                routes::payments::handle_bic_generate
            ),
            (
                "paymentsBicValidate",
                Post,
                routes::payments::handle_bic_validate
            ),
            (
                "paymentsCreditorIdGenerate",
                Post,
                routes::payments::handle_creditor_id_generate
            ),
            (
                "paymentsCreditorIdValidate",
                Post,
                routes::payments::handle_creditor_id_validate
            ),
            (
                "paymentsMandateReferenceGenerate",
                Post,
                routes::payments::handle_mandate_reference_generate
            ),
            (
                "paymentsMandateReferenceValidate",
                Post,
                routes::payments::handle_mandate_reference_validate
            ),
            (
                "paymentsEndToEndIdGenerate",
                Post,
                routes::payments::handle_end_to_end_id_generate
            ),
            (
                "paymentsEndToEndIdValidate",
                Post,
                routes::payments::handle_end_to_end_id_validate
            ),
            (
                "paymentsRfReferenceGenerate",
                Post,
                routes::payments::handle_rf_reference_generate
            ),
            (
                "paymentsRfReferenceValidate",
                Post,
                routes::payments::handle_rf_reference_validate
            ),
            (
                "paymentsUetrGenerate",
                Post,
                routes::payments::handle_uetr_generate
            ),
            (
                "paymentsUetrValidate",
                Post,
                routes::payments::handle_uetr_validate
            ),
            (
                "businessVatIdValidate",
                Post,
                routes::business::handle_vat_id_validate
            ),
            (
                "businessLeiValidate",
                Post,
                routes::business::handle_lei_validate
            ),
            (
                "businessLeiLookup",
                Post,
                routes::business::handle_lei_lookup
            ),
            (
                "energyMastrGenerate",
                Post,
                routes::registers::handle_mastr_generate
            ),
            (
                "energyMastrValidate",
                Post,
                routes::registers::handle_mastr_validate
            ),
            (
                "energyEicValidate",
                Post,
                routes::registers::handle_eic_validate
            ),
            (
                "energyEicLookup",
                Post,
                routes::registers::handle_eic_lookup
            ),
            (
                "meteringObisValidate",
                Post,
                routes::metering::handle_obis_validate
            ),
            (
                "meteringObisLookup",
                Post,
                routes::metering::handle_obis_lookup
            ),
            (
                "meteringDin43849Validate",
                Post,
                routes::metering::handle_din_43849_validate
            ),
            (
                "testDataNegativeMaloGenerate",
                Post,
                routes::negative::handle_malo_negative_generate
            ),
            (
                "testDataNegativeMeloGenerate",
                Post,
                routes::negative::handle_melo_negative_generate
            ),
            (
                "testDataNegativeNeloGenerate",
                Post,
                routes::negative::handle_nelo_negative_generate
            ),
            (
                "testDataNegativeNebeGenerate",
                Post,
                routes::negative::handle_nebe_negative_generate
            ),
            (
                "testDataNegativeMarketPartnerIdGenerate",
                Post,
                routes::negative::handle_market_partner_negative_generate
            ),
            (
                "testDataNegativeClusterResourceIdGenerate",
                Post,
                routes::negative::handle_cr_negative_generate
            ),
            (
                "testDataNegativeSteeringGroupIdGenerate",
                Post,
                routes::negative::handle_sg_negative_generate
            ),
            (
                "testDataNegativeControllableResourceIdGenerate",
                Post,
                routes::negative::handle_sr_negative_generate
            ),
            (
                "testDataNegativeTechnicalResourceIdGenerate",
                Post,
                routes::negative::handle_tr_negative_generate
            ),
            (
                "testDataNegativePackageIdGenerate",
                Post,
                routes::negative::handle_package_negative_generate
            ),
            (
                "testDataNegativeIbanGenerate",
                Post,
                routes::negative::handle_iban_negative_generate
            ),
            (
                "testDataNegativeBicGenerate",
                Post,
                routes::negative::handle_bic_negative_generate
            ),
            (
                "testDataNegativeCreditorIdGenerate",
                Post,
                routes::negative::handle_creditor_id_negative_generate
            ),
            (
                "testDataNegativeMandateReferenceGenerate",
                Post,
                routes::negative::handle_mandate_reference_negative_generate
            ),
            (
                "testDataNegativeEndToEndIdGenerate",
                Post,
                routes::negative::handle_end_to_end_id_negative_generate
            ),
            (
                "testDataNegativeRfReferenceGenerate",
                Post,
                routes::negative::handle_rf_reference_negative_generate
            ),
            (
                "testDataNegativeUetrGenerate",
                Post,
                routes::negative::handle_uetr_negative_generate
            ),
            (
                "testDataNegativeMastrGenerate",
                Post,
                routes::negative::handle_mastr_negative_generate
            ),
            (
                "testDataNegativeVatIdGenerate",
                Post,
                routes::negative::handle_vat_id_negative_generate
            ),
            (
                "testDataNegativeLeiGenerate",
                Post,
                routes::negative::handle_lei_negative_generate
            ),
            (
                "testDataNegativeEicGenerate",
                Post,
                routes::negative::handle_eic_negative_generate
            ),
            (
                "testDataNegativeObisGenerate",
                Post,
                routes::negative::handle_obis_negative_generate
            ),
            (
                "testDataNegativeDin43849Generate",
                Post,
                routes::negative::handle_din_43849_negative_generate
            ),
            ("systemCatalogList", Get, catalog_api::handle_catalog),
            (
                "testDataScenarioCatalog",
                Get,
                routes::scenarios::handle_scenarios
            ),
            (
                "testDataScenarioGenerate",
                Post,
                routes::scenarios::handle_scenario_generate
            ),
        );
    };
}

struct HandlerRegistration {
    operation_id: &'static str,
    declared_method: ApiMethod,
    method_router: MethodRouter,
}

macro_rules! define_api_doc_and_handler_registry {
    ($(($operation_id:literal, $method:ident, $handler:path)),+ $(,)?) => {
        #[derive(OpenApi)]
        #[openapi(paths($($handler),+))]
        struct ApiDoc;

        fn handler_registry() -> Vec<HandlerRegistration> {
            vec![$(
                {
                    let operation = catalog_operation_by_id($operation_id).unwrap_or_else(|| {
                        panic!("HTTP handler '{}' has no catalog operation", $operation_id)
                    });
                    HandlerRegistration {
                        operation_id: $operation_id,
                        declared_method: ApiMethod::$method,
                        // The catalog selects the effective HTTP method. The
                        // macro method is retained solely for consistency tests.
                        method_router: match operation.method {
                            ApiMethod::Get => get($handler),
                            ApiMethod::Post => post($handler),
                        },
                    }
                }
            ),+]
        }
    };
}

public_operation_handlers!(define_api_doc_and_handler_registry);

#[derive(OpenApi)]
#[openapi(
    components(schemas(
        ValidateRequest,
        GenerateMaloResponse,
        ValidateMaloResponse,
        GenerateMeloResponse,
        ValidateMeloResponse,
        GenerateNeloResponse,
        ValidateNeloResponse,
        ErrorResponse,
        GenerateRequest,
        GenerateResponse,
        GeneratedIdentifier,
        ValidationReport,
        IdentifierKind,
        GenerationProfile,
        IdentifierFormat,
        Sector,
        CheckStatus,
        Checks,
        AllocationStatus,
        AccountExistenceStatus,
        CollisionGuarantee,
        IdentifierPart,
        ReferenceData,
        BicGenerateRequest,
        RfGenerateRequest,
        routes::business::LeiLookupStatus,
        routes::business::LeiLookupCacheStatus,
        routes::business::LeiLookupResponse,
        routes::registers::MastrGenerateRequest,
        routes::registers::EicLookupStatus,
        routes::registers::EicLookupResponse,
        routes::metering::ObisLookupStatus,
        routes::metering::ObisCatalogLookupEntry,
        routes::metering::ObisLookupResponse,
        routes::negative::NegativeMutation,
        routes::negative::NegativeFixtureRequest,
        routes::negative::NegativeFixtureResponse,
        routes::scenarios::ScenarioKind,
        routes::scenarios::ScenarioCatalogResponse,
        routes::scenarios::ScenarioDescriptor,
        routes::scenarios::ScenarioIdentifierDescriptor,
        routes::scenarios::ScenarioGenerateRequest,
        routes::scenarios::ScenarioGenerateResponse,
        routes::scenarios::ScenarioGeneratedItem,
        CatalogResponse,
        CatalogIdentifier,
        CatalogOperation,
        CatalogExample,
        CatalogSource
    )),
    tags(
        (name = "Energie · Marktpartner", description = "Marktpartner-Identifikationsnummern"),
        (name = "Energie · Lokationen", description = "Markt-, Mess- und Netzlokationen"),
        (name = "Energie · Ressourcen & Redispatch", description = "Ressourcen für Redispatch und Netzbetreiberkoordination"),
        (name = "Energie · Register & Anlagen", description = "Register- und Anlagenkennungen"),
        (name = "Messwesen · Geräte & Werte", description = "Gerätekennungen, Messwerte und OBIS"),
        (name = "Zahlungsverkehr · Konten & Banken", description = "Konten- und Institutskennungen"),
        (name = "Zahlungsverkehr · SEPA & Referenzen", description = "SEPA- und Zahlungsreferenzen"),
        (name = "Unternehmen · Stammdaten & Register", description = "Unternehmens-, Steuer- und Registerkennungen"),
        (name = "Testdaten · Szenarien", description = "Zusammenhängende Testdatensätze und Szenarien"),
        (name = "System · Katalog", description = "Maschinenlesbarer Kennungskatalog")
    ),
    info(
        title = "NRG ID Generator API",
        version = "1.0.0",
        description = "Generate and validate German energy-market and payment identifiers. Embedded reference-data versions are added from the compiled snapshot."
    )
)]
struct ApiMetadataDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut document = ApiMetadataDoc::openapi();
    document.merge(ApiDoc::openapi());
    utoipa::Modify::modify(&catalog_api::CatalogOpenApiModifier, &mut document);
    document
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

fn catalog_operations() -> impl Iterator<Item = &'static ApiOperationDescriptor> {
    id_core::catalog::identifier_catalog()
        .iter()
        .flat_map(|descriptor| descriptor.operations.iter())
        .chain(
            id_core::catalog::service_operations()
                .iter()
                .map(|service| &service.operation),
        )
}

fn catalog_operation_by_id(operation_id: &str) -> Option<&'static ApiOperationDescriptor> {
    catalog_operations().find(|operation| operation.operation_id == operation_id)
}

fn catalog_router() -> Router {
    let mut router = Router::new();
    for registration in handler_registry() {
        let operation = catalog_operation_by_id(registration.operation_id).unwrap_or_else(|| {
            panic!(
                "HTTP handler '{}' has no catalog operation",
                registration.operation_id
            )
        });
        assert_eq!(
            operation.method, registration.declared_method,
            "HTTP handler '{}' declares a method different from the catalog",
            registration.operation_id
        );

        router = router.route(operation.path, registration.method_router);
    }
    router
}

/// Builds the platform-neutral Axum router shared by both deployment targets.
///
/// Every public API route is assembled from the central catalog. The OpenAPI
/// document endpoint itself is intentionally infrastructure metadata and stays
/// outside the identifier/service catalog.
pub fn router() -> Router {
    catalog_router()
        .route(OPENAPI_JSON_PATH, get(openapi_json))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([CONTENT_TYPE]),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    const MAX_TEST_RESPONSE_BYTES: usize = 1024 * 1024;

    async fn send_json(method: Method, path: &str, body: Value) -> (StatusCode, Value) {
        let response = router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), MAX_TEST_RESPONSE_BYTES)
            .await
            .unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    async fn generate(path: &str) -> Value {
        let response = router()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), MAX_TEST_RESPONSE_BYTES)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn generate_v1(path: &str, options: Value) -> Value {
        let (status, body) = send_json(Method::POST, path, options).await;
        assert_eq!(status, StatusCode::OK, "path: {path}, body: {body}");
        body
    }

    #[tokio::test]
    async fn generated_ids_revalidate_through_shared_core() {
        let malo = generate("/api/malo/generate").await;
        assert!(validate_malo(malo["id"].as_str().unwrap()).is_ok());

        let melo = generate("/api/melo/generate").await;
        assert!(validate_melo(melo["id"].as_str().unwrap()).is_ok());

        let nelo = generate("/api/nelo/generate").await;
        assert!(validate_nelo(nelo["id"].as_str().unwrap()).is_ok());
    }

    #[tokio::test]
    async fn every_v1_generator_returns_values_accepted_by_its_core_validator() {
        use id_core::identifiers::{
            energy::{
                validate_cr_id, validate_market_partner_id, validate_nebe_id, validate_package_id,
                validate_sg_id, validate_sr_id, validate_tr_id,
            },
            payments::{
                bic::validate_bic, creditor_id::validate_german_creditor_id,
                end_to_end_id::validate_end_to_end_id, iban::validate_german_iban,
                mandate_reference::validate_mandate_reference, rf_reference::validate_rf_reference,
                uetr::validate_uetr,
            },
            registers::validate_mastr,
        };

        let cases = [
            ("/api/v1/energy/locations/malo/generate", "malo"),
            ("/api/v1/energy/locations/melo/generate", "melo"),
            ("/api/v1/energy/locations/nelo/generate", "nelo"),
            ("/api/v1/energy/locations/nebe/generate", "nebe"),
            ("/api/v1/energy/market-partners/mp-id/generate", "mp-id"),
            ("/api/v1/energy/resources/cr-id/generate", "cr-id"),
            ("/api/v1/energy/resources/sg-id/generate", "sg-id"),
            ("/api/v1/energy/resources/sr-id/generate", "sr-id"),
            ("/api/v1/energy/resources/tr-id/generate", "tr-id"),
            ("/api/v1/energy/resources/package-id/generate", "package-id"),
            ("/api/v1/payments/accounts/iban/generate", "iban"),
            ("/api/v1/payments/institutions/bic/generate", "bic"),
            ("/api/v1/payments/sepa/creditor-id/generate", "creditor-id"),
            (
                "/api/v1/payments/sepa/mandate-reference/generate",
                "mandate-reference",
            ),
            (
                "/api/v1/payments/sepa/end-to-end-id/generate",
                "end-to-end-id",
            ),
            (
                "/api/v1/payments/sepa/rf-reference/generate",
                "rf-reference",
            ),
            ("/api/v1/payments/sepa/uetr/generate", "uetr"),
            ("/api/v1/energy/registers/mastr/generate", "mastr"),
        ];

        for (path, kind) in cases {
            let mut options = json!({
                "count": 3,
                "fixture_seed": "api-self-validation"
            });
            if kind == "mp-id" {
                options["sector"] = json!("electricity");
            }
            let response = generate_v1(path, options).await;
            assert_eq!(response["generator_version"], id_core::GENERATOR_VERSION);
            assert_eq!(response["items"].as_array().unwrap().len(), 3);

            for item in response["items"].as_array().unwrap() {
                assert_eq!(item["kind"], kind, "path: {path}");
                assert_eq!(item["production_usable"], false, "path: {path}");
                assert_eq!(item["checks"]["syntax"], "valid", "path: {path}");
                let value = item["value"].as_str().unwrap();
                let accepted = match kind {
                    "malo" => validate_malo(value).is_ok(),
                    "melo" => validate_melo(value).is_ok(),
                    "nelo" => validate_nelo(value).is_ok(),
                    "nebe" => validate_nebe_id(value).is_ok(),
                    "mp-id" => validate_market_partner_id(value).is_ok(),
                    "cr-id" => validate_cr_id(value).is_ok(),
                    "sg-id" => validate_sg_id(value).is_ok(),
                    "sr-id" => validate_sr_id(value).is_ok(),
                    "tr-id" => validate_tr_id(value).is_ok(),
                    "package-id" => validate_package_id(value).is_ok(),
                    "iban" => validate_german_iban(value).is_ok(),
                    "bic" => validate_bic(value).is_ok(),
                    "creditor-id" => validate_german_creditor_id(value).is_ok(),
                    "mandate-reference" => validate_mandate_reference(value).is_ok(),
                    "end-to-end-id" => validate_end_to_end_id(value).is_ok(),
                    "rf-reference" => validate_rf_reference(value).is_ok(),
                    "uetr" => validate_uetr(value).is_ok(),
                    "mastr" => validate_mastr(value).is_ok(),
                    _ => unreachable!(),
                };
                assert!(accepted, "{kind} generator returned invalid value {value}");
            }
        }
    }

    #[tokio::test]
    async fn non_default_profile_and_format_matrix_revalidates() {
        for (profile, expected_directory) in [
            ("synthetic_non_routable", "not_found"),
            ("directory_plausible", "found"),
            ("checksum_only", "not_checked"),
        ] {
            for format in ["electronic", "formatted"] {
                let response = generate_v1(
                    "/api/v1/payments/accounts/iban/generate",
                    json!({
                        "profile": profile,
                        "format": format,
                        "count": 3,
                        "fixture_seed": "iban-profile-matrix"
                    }),
                )
                .await;
                for item in response["items"].as_array().unwrap() {
                    assert_eq!(item["profile"], profile);
                    assert_eq!(item["checks"]["directory"], expected_directory);
                    assert!(id_core::identifiers::payments::iban::validate_german_iban(
                        item["value"].as_str().unwrap()
                    )
                    .is_ok());
                    assert_eq!(
                        item["value"].as_str().unwrap().contains(' '),
                        format == "formatted"
                    );
                }
            }
        }

        let gas = generate_v1(
            "/api/v1/energy/market-partners/mp-id/generate",
            json!({
                "sector": "gas",
                "count": 3,
                "fixture_seed": "gas-market-partner"
            }),
        )
        .await;
        for item in gas["items"].as_array().unwrap() {
            assert!(id_core::identifiers::energy::validate_market_partner_id(
                item["value"].as_str().unwrap()
            )
            .is_ok());
            assert!(item["parts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|part| part["name"] == "sector" && part["value"] == "gas"));
        }

        for profile in ["test_training_pattern", "syntax_only", "directory_value"] {
            let response = generate_v1(
                "/api/v1/payments/institutions/bic/generate",
                json!({
                    "profile": profile,
                    "include_branch": true,
                    "count": 3,
                    "fixture_seed": "bic-branch-matrix"
                }),
            )
            .await;
            for item in response["items"].as_array().unwrap() {
                assert_eq!(item["value"].as_str().unwrap().len(), 11);
                assert!(id_core::identifiers::payments::bic::validate_bic(
                    item["value"].as_str().unwrap()
                )
                .is_ok());
            }
        }
    }

    #[tokio::test]
    async fn international_iban_profiles_use_the_versioned_swift_registry() {
        use id_core::identifiers::payments::international_iban::validate_international_iban;

        for (country, profile, synthetic) in [
            ("GB", "official_example", false),
            ("FR", "checksum_only", true),
            ("KW", "official_example", false),
        ] {
            let generated = generate_v1(
                "/api/v1/payments/accounts/iban/generate",
                json!({
                    "country": country,
                    "profile": profile,
                    "format": "formatted",
                    "count": 2,
                    "fixture_seed": "international-iban-api"
                }),
            )
            .await;
            for item in generated["items"].as_array().unwrap() {
                let value = item["value"].as_str().unwrap();
                let parsed = validate_international_iban(value).unwrap();
                assert_eq!(parsed.country_code, country);
                assert_eq!(item["synthetic"], synthetic);
                assert_eq!(item["production_usable"], false);
                assert_eq!(item["checks"]["checksum"], "valid");
                assert_eq!(item["reference_data"]["version"], "release-102");

                let (status, validation) = send_json(
                    Method::POST,
                    "/api/v1/payments/accounts/iban/validate",
                    json!({ "id": value }),
                )
                .await;
                assert_eq!(status, StatusCode::OK);
                assert_eq!(validation["valid"], true);
                assert_eq!(validation["checks"]["directory"], "not_checked");
            }
        }

        let (status, _) = send_json(
            Method::POST,
            "/api/v1/payments/accounts/iban/generate",
            json!({ "country": "ZZ", "fixture_seed": "unknown-country" }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn seeded_batches_are_reproducible_and_profile_semantics_are_explicit() {
        let request = json!({
            "profile": "synthetic_non_routable",
            "count": 5,
            "fixture_seed": "integration-test-4711",
            "format": "electronic"
        });
        let first = generate_v1("/api/v1/payments/accounts/iban/generate", request.clone()).await;
        let second = generate_v1("/api/v1/payments/accounts/iban/generate", request).await;
        assert_eq!(first, second);

        for item in first["items"].as_array().unwrap() {
            assert_eq!(item["profile"], "synthetic_non_routable");
            assert_eq!(item["synthetic"], true);
            assert_eq!(item["production_usable"], false);
            assert_eq!(item["checks"]["checksum"], "valid");
            assert_eq!(item["checks"]["directory"], "not_found");
            assert_eq!(item["account_existence"], "unknown");
            assert_eq!(item["collision_guarantee"], "none");
            assert_eq!(item["reference_data"]["name"], "bundesbank_blz");
            let bank_code = item["parts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|part| part["name"] == "bank_code")
                .unwrap()["value"]
                .as_str()
                .unwrap();
            assert!(!id_core::reference_data::BundesbankBlzDirectory.contains_bank_code(bank_code));
        }
    }

    #[tokio::test]
    async fn scenario_generation_is_reproducible_across_all_dependencies() {
        let request = json!({
            "scenario": "supplier_direct_debit",
            "sector": "electricity",
            "profile": "synthetic_non_routable",
            "fixture_seed": "nrg-scenario-contract"
        });
        let first = generate_v1("/api/v1/scenarios/generate", request.clone()).await;
        let second = generate_v1("/api/v1/scenarios/generate", request).await;
        assert_eq!(first, second);
        assert_eq!(first["generator_version"], id_core::GENERATOR_VERSION);
        assert_eq!(first["items"].as_array().unwrap().len(), 8);
        assert!(first["items"].as_array().unwrap().iter().any(|item| {
            item["identifier"]["kind"] == "mastr"
                && item["identifier"]["production_usable"] == false
        }));
    }

    #[tokio::test]
    async fn catalog_is_available_and_generation_options_are_bounded() {
        let catalog = generate("/api/v1/catalog").await;
        assert_eq!(
            catalog["catalog_version"],
            id_core::catalog::CATALOG_VERSION
        );
        assert_eq!(
            catalog["identifiers"].as_array().unwrap().len(),
            id_core::catalog::identifier_catalog().len()
        );
        assert_eq!(
            catalog["operations"].as_array().unwrap().len(),
            id_core::catalog::service_operations().len()
        );

        for count in [0, 101] {
            let (status, body) = send_json(
                Method::POST,
                "/api/v1/payments/accounts/iban/generate",
                json!({ "count": count, "fixture_seed": "bounded" }),
            )
            .await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert!(body["error"].as_str().unwrap().contains("count"));
        }
    }

    #[test]
    fn handler_registry_and_catalog_are_bidirectionally_complete() {
        use std::collections::HashMap;

        let catalog: Vec<_> = catalog_operations().collect();
        let registry = handler_registry();
        let mut handler_counts = HashMap::<&str, usize>::new();

        for registration in &registry {
            *handler_counts.entry(registration.operation_id).or_default() += 1;
            let matches: Vec<_> = catalog
                .iter()
                .copied()
                .filter(|operation| operation.operation_id == registration.operation_id)
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "registry handler '{}' must map to exactly one catalog operation",
                registration.operation_id
            );
            assert_eq!(
                matches[0].method, registration.declared_method,
                "registry method differs from catalog for '{}'",
                registration.operation_id
            );
        }

        for operation in &catalog {
            assert_eq!(
                handler_counts.get(operation.operation_id),
                Some(&1),
                "catalog operation '{}' must have exactly one handler",
                operation.operation_id
            );
            assert_ne!(operation.path, OPENAPI_JSON_PATH);
        }
        assert_eq!(registry.len(), catalog.len());
        assert!(!handler_counts.contains_key("openapiJson"));
    }

    #[tokio::test]
    async fn every_catalog_operation_is_reachable_through_the_router() {
        use id_core::catalog::{ApiMethod, Capability};

        for descriptor in id_core::catalog::identifier_catalog() {
            for operation in descriptor.operations {
                let mut request = Request::builder()
                    .method(match operation.method {
                        ApiMethod::Get => Method::GET,
                        ApiMethod::Post => Method::POST,
                    })
                    .uri(operation.path);
                let body = if operation.method == ApiMethod::Post {
                    request = request.header(CONTENT_TYPE, "application/json");
                    match operation.capability {
                        Capability::Generate => {
                            Body::from(r#"{"fixture_seed":"catalog-reachability"}"#)
                        }
                        Capability::NegativeFixture => Body::from(
                            r#"{"mutation":"length","fixture_seed":"catalog-reachability"}"#,
                        ),
                        _ => Body::from(r#"{"id":"invalid-test-value"}"#),
                    }
                } else {
                    Body::empty()
                };

                let response = router().oneshot(request.body(body).unwrap()).await.unwrap();
                assert!(
                    !matches!(
                        response.status(),
                        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
                    ),
                    "catalog operation is not reachable: {:?} {}",
                    operation.method,
                    operation.path
                );
            }
        }

        for service in id_core::catalog::service_operations() {
            let operation = &service.operation;
            let mut request = Request::builder()
                .method(match operation.method {
                    ApiMethod::Get => Method::GET,
                    ApiMethod::Post => Method::POST,
                })
                .uri(operation.path);
            let body = if operation.method == ApiMethod::Post {
                request = request.header(CONTENT_TYPE, "application/json");
                Body::from(
                    r#"{"scenario":"supplier_basic","sector":"electricity","fixture_seed":"catalog-reachability"}"#,
                )
            } else {
                Body::empty()
            };
            let response = router().oneshot(request.body(body).unwrap()).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{}", operation.path);
        }
    }

    #[tokio::test]
    async fn payment_specific_contracts_remain_distinct() {
        let bic = generate_v1(
            "/api/v1/payments/institutions/bic/generate",
            json!({
                "profile": "test_training_pattern",
                "fixture_seed": "bic-pattern",
                "include_branch": true
            }),
        )
        .await;
        let bic_value = bic["items"][0]["value"].as_str().unwrap();
        assert_eq!(bic_value.len(), 11);
        assert_eq!(bic_value.as_bytes()[7], b'0');
        assert_eq!(bic["items"][0]["checks"]["checksum"], "not_applicable");

        for profile in ["test_training_pattern", "syntax_only", "directory_value"] {
            let batch = generate_v1(
                "/api/v1/payments/institutions/bic/generate",
                json!({
                    "profile": profile,
                    "count": 100,
                    "fixture_seed": "unique-bic-batch"
                }),
            )
            .await;
            let values: std::collections::HashSet<_> = batch["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["value"].as_str().unwrap())
                .collect();
            assert_eq!(values.len(), 100, "duplicate BIC in {profile} profile");

            if profile == "directory_value" {
                let directory = id_core::reference_data::BundesbankBlzDirectory;
                for item in batch["items"].as_array().unwrap() {
                    let bank_code = item["parts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|part| part["name"] == "bank_code")
                        .and_then(|part| part["value"].as_str())
                        .expect("directory BIC must expose its bank code");
                    let record = directory.lookup(bank_code).unwrap();
                    assert_eq!(&record.bic.unwrap()[..8], item["value"].as_str().unwrap());
                }
            }
        }

        let creditor = generate_v1(
            "/api/v1/payments/sepa/creditor-id/generate",
            json!({ "fixture_seed": "ignored-for-official-fixture" }),
        )
        .await;
        assert_eq!(creditor["items"][0]["value"], "DE98ZZZ09999999999");
        assert_eq!(
            creditor["items"][0]["reference_data"]["name"],
            "bundesbank_creditor_id"
        );

        for path in [
            "/api/v1/payments/sepa/mandate-reference/generate",
            "/api/v1/payments/sepa/end-to-end-id/generate",
        ] {
            let response = generate_v1(
                path,
                json!({ "count": 100, "fixture_seed": "unique-batch" }),
            )
            .await;
            let values: std::collections::HashSet<_> = response["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item["value"].as_str().unwrap())
                .collect();
            assert_eq!(values.len(), 100);
            assert!(values.iter().all(|value| value.len() <= 35));
            assert!(response["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["collision_guarantee"] == "within_batch"));
        }

        let rf = generate_v1(
            "/api/v1/payments/sepa/rf-reference/generate",
            json!({
                "fixture_seed": "rf-explicit",
                "invoice_reference": "NRG202600001234"
            }),
        )
        .await;
        let rf_value = rf["items"][0]["value"].as_str().unwrap();
        assert!(
            id_core::identifiers::payments::rf_reference::validate_rf_reference(rf_value).is_ok()
        );
    }

    #[tokio::test]
    async fn public_sepa_reference_validators_report_precise_non_allocated_semantics() {
        let (status, mandate) = send_json(
            Method::POST,
            "/api/v1/payments/sepa/mandate-reference/validate",
            json!({ "id": "NRG-MND-CUSTOMER-4711" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(mandate["valid"], true);
        assert_eq!(mandate["normalized"], "NRG-MND-CUSTOMER-4711");
        assert_eq!(mandate["checks"]["checksum"], "not_applicable");
        assert_eq!(mandate["checks"]["directory"], "not_applicable");
        assert_eq!(mandate["checks"]["assignment"], "not_applicable");
        assert_eq!(mandate["allocation_status"], "not_applicable");

        let (status, not_provided) = send_json(
            Method::POST,
            "/api/v1/payments/sepa/end-to-end-id/validate",
            json!({ "id": "NOTPROVIDED" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(not_provided["valid"], true);
        assert_eq!(not_provided["normalized"], "NOTPROVIDED");
        assert_eq!(not_provided["checks"]["checksum"], "not_applicable");
        assert_eq!(not_provided["allocation_status"], "not_applicable");
        assert_eq!(
            not_provided["parts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|part| part["name"] == "not_provided")
                .unwrap()["value"],
            "true"
        );

        for path in [
            "/api/v1/payments/sepa/mandate-reference/validate",
            "/api/v1/payments/sepa/end-to-end-id/validate",
        ] {
            let (status, invalid) =
                send_json(Method::POST, path, json!({ "id": "X".repeat(36) })).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(invalid["valid"], false);
            assert_eq!(invalid["checks"]["syntax"], "invalid");
            assert_eq!(invalid["checks"]["checksum"], "not_applicable");
            assert_eq!(invalid["allocation_status"], "not_applicable");
        }
    }

    #[tokio::test]
    async fn validator_only_negative_fixture_routes_self_verify_applicable_mutations() {
        for path in [
            "/api/v1/test-data/negative/vat-id/generate",
            "/api/v1/test-data/negative/lei/generate",
            "/api/v1/test-data/negative/eic/generate",
            "/api/v1/test-data/negative/obis/generate",
            "/api/v1/test-data/negative/din-43849/generate",
        ] {
            for mutation in ["length", "character_set"] {
                let (status, body) = send_json(
                    Method::POST,
                    path,
                    json!({
                        "mutation": mutation,
                        "fixture_seed": "validator-only-negative"
                    }),
                )
                .await;
                assert_eq!(status, StatusCode::OK, "{path} {mutation}");
                assert_eq!(body["expected_valid"], false);
                assert_eq!(body["validator_rejected"], true);
                assert_eq!(body["original"]["production_usable"], false);
            }
        }

        for path in [
            "/api/v1/test-data/negative/lei/generate",
            "/api/v1/test-data/negative/eic/generate",
        ] {
            let (status, body) = send_json(
                Method::POST,
                path,
                json!({ "mutation": "checksum", "fixture_seed": "checksum-negative" }),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert_eq!(body["validator_rejected"], true);
            assert_ne!(body["original"]["value"], body["mutated_value"]);
        }

        let (status, body) = send_json(
            Method::POST,
            "/api/v1/test-data/negative/vat-id/generate",
            json!({ "mutation": "checksum" }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("no standardized checksum"));
    }

    #[tokio::test]
    async fn validation_routes_report_valid_and_invalid_ids() {
        let cases = [
            ("/api/malo/validate", "41373559241", "41373559240"),
            (
                "/api/melo/validate",
                "DE00056266802AO6G56M11SN51G21M24S",
                "DE00056266802ao6g56m11sn51g21m24s",
            ),
            ("/api/nelo/validate", "EABC123DEF8", "EABC123DEF0"),
        ];

        for (path, valid, invalid) in cases {
            let (status, body) = send_json(Method::POST, path, json!({ "id": valid })).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["valid"], true);

            let (status, body) = send_json(Method::POST, path, json!({ "id": invalid })).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["valid"], false);
            assert!(body["error"].is_string());
        }
    }

    #[tokio::test]
    async fn validator_warnings_do_not_label_caller_values_as_synthetic() {
        for (path, value) in [
            ("/api/v1/energy/locations/malo/validate", "41373559241"),
            (
                "/api/v1/payments/accounts/iban/validate",
                "DE79000000001234567890",
            ),
        ] {
            let (status, body) = send_json(Method::POST, path, json!({ "id": value })).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["valid"], true);
            assert_eq!(body["synthetic"], serde_json::Value::Null);
            let warnings = body["warnings"].as_array().unwrap();
            assert!(warnings.iter().all(|warning| {
                let warning = warning.as_str().unwrap().to_ascii_lowercase();
                !warning.contains("synthetic") && !warning.contains("test value")
            }));
        }
    }

    #[tokio::test]
    async fn invalid_rf_reference_keeps_non_applicable_allocation_semantics() {
        let mut invalid =
            id_core::identifiers::payments::rf_reference::build_rf_reference("NRG202600001234")
                .unwrap()
                .value;
        let replacement = if &invalid[2..3] == "0" { "1" } else { "0" };
        invalid.replace_range(2..3, replacement);
        let (status, body) = send_json(
            Method::POST,
            "/api/v1/payments/sepa/rf-reference/validate",
            json!({ "id": invalid }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["valid"], false);
        assert_eq!(body["checks"]["checksum"], "invalid");
        assert_eq!(body["checks"]["directory"], "not_applicable");
        assert_eq!(body["checks"]["assignment"], "not_applicable");
        assert_eq!(body["allocation_status"], "not_applicable");
    }

    #[tokio::test]
    async fn json_rejections_have_precise_status_and_json_body() {
        let cases = [
            (Some("application/json"), "{", StatusCode::BAD_REQUEST),
            (
                Some("application/json"),
                "{}",
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                None,
                r#"{"id":"41373559241"}"#,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
        ];

        for path in [
            "/api/malo/validate",
            "/api/melo/validate",
            "/api/nelo/validate",
        ] {
            for (content_type, body, expected_status) in cases {
                let mut request = Request::builder().method(Method::POST).uri(path);
                if let Some(content_type) = content_type {
                    request = request.header(CONTENT_TYPE, content_type);
                }
                let response = router()
                    .oneshot(request.body(Body::from(body)).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), expected_status, "path: {path}");
                assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
                let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
                let body: ErrorResponse = serde_json::from_slice(&body).unwrap();
                assert!(!body.error.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn oversized_requests_are_rejected() {
        for path in [
            "/api/malo/validate",
            "/api/melo/validate",
            "/api/nelo/validate",
        ] {
            let (status, body) = send_json(
                Method::POST,
                path,
                json!({ "id": "1".repeat(MAX_REQUEST_BODY_BYTES + 1) }),
            )
            .await;
            assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "path: {path}");
            assert_eq!(
                body["error"],
                format!("Request body exceeds {MAX_REQUEST_BODY_BYTES} bytes")
            );
        }
    }

    #[tokio::test]
    async fn non_ascii_ids_are_rejected_without_panicking() {
        for (path, id) in [
            ("/api/malo/validate", "4é13735592"),
            ("/api/melo/validate", "DE00056é66802AO6G56M11SN51G21M2"),
            ("/api/nelo/validate", "EABC12éDEF8"),
        ] {
            let (status, body) = send_json(Method::POST, path, json!({ "id": id })).await;
            assert_eq!(status, StatusCode::OK, "path: {path}");
            assert_eq!(body["valid"], false, "path: {path}");
            assert!(body["error"].is_string(), "path: {path}");
        }
    }

    #[tokio::test]
    async fn cors_preflight_is_handled_by_the_shared_router() {
        let response = router()
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/api/malo/validate")
                    .header(header::ORIGIN, "https://example.com")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    }

    #[tokio::test]
    async fn lei_rate_limit_response_has_retry_contract_without_identifier_data() {
        let response = lei_lookup_rate_limit_response(60);
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "60");
        let body = to_bytes(response.into_body(), MAX_TEST_RESPONSE_BYTES)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["retry_after_seconds"], 60);
        assert_eq!(body["error"], "LEI lookup rate limit exceeded; retry later");
        assert!(!body.to_string().contains("506700GE1G29325QX363"));
    }

    #[test]
    fn catalog_and_openapi_operations_are_consistent() {
        let document = serde_json::to_value(openapi()).unwrap();
        let paths = document["paths"].as_object().unwrap();
        assert!(catalog_api::missing_catalog_operations(&openapi()).is_empty());

        let mut expected_operations = std::collections::HashSet::new();
        let mut operation_ids = std::collections::HashSet::new();
        for descriptor in id_core::catalog::identifier_catalog() {
            assert!(!descriptor.operations.is_empty(), "{}", descriptor.slug);
            for operation in descriptor.operations {
                let method = operation.method.as_str();
                let operation_json = &paths[operation.path][method];
                assert!(operation_json.is_object(), "{} {method}", operation.path);
                assert_eq!(operation_json["operationId"], operation.operation_id);
                assert_eq!(
                    operation_json["tags"].as_array().unwrap(),
                    &[Value::String(operation.primary_tag.to_string())]
                );
                for extension in [
                    "x-nrg-domain",
                    "x-nrg-roles",
                    "x-nrg-sectors",
                    "x-nrg-capabilities",
                    "x-nrg-allocation-model",
                    "x-nrg-generation-profiles",
                    "x-nrg-format",
                    "x-nrg-sources",
                ] {
                    assert!(
                        !operation_json[extension].is_null(),
                        "missing {extension} on {} {method}",
                        operation.path
                    );
                }
                assert_eq!(
                    operation_json["deprecated"].as_bool().unwrap_or(false),
                    operation.deprecated,
                    "{} {method}",
                    operation.path
                );
                assert!(operation_ids.insert(operation.operation_id));
                expected_operations.insert((operation.path, method));

                if !operation.deprecated && operation.path.starts_with("/api/v1/") {
                    let responses = operation_json["responses"].as_object().unwrap();
                    let expected_statuses: &[&str] =
                        if operation.operation_id == "businessLeiLookup" {
                            &["200", "400", "413", "415", "422", "429", "502"]
                        } else if matches!(
                            operation.capability,
                            id_core::catalog::Capability::Generate
                                | id_core::catalog::Capability::NegativeFixture
                        ) {
                            &["200", "400", "413", "415", "422", "500"]
                        } else {
                            &["200", "400", "413", "415", "422"]
                        };
                    assert_eq!(
                        responses.len(),
                        expected_statuses.len(),
                        "{}",
                        operation.path
                    );
                    for status in expected_statuses {
                        assert!(
                            responses.contains_key(*status),
                            "missing response {status} on {}",
                            operation.path
                        );
                    }
                }
            }
        }

        for service in id_core::catalog::service_operations() {
            let operation = &service.operation;
            let method = operation.method.as_str();
            let operation_json = &paths[operation.path][method];
            assert!(operation_json.is_object(), "{} {method}", operation.path);
            assert_eq!(operation_json["operationId"], operation.operation_id);
            assert_eq!(operation_json["tags"], json!([operation.primary_tag]));
            for extension in [
                "x-nrg-domain",
                "x-nrg-roles",
                "x-nrg-sectors",
                "x-nrg-capabilities",
                "x-nrg-allocation-model",
                "x-nrg-generation-profiles",
            ] {
                assert!(
                    !operation_json[extension].is_null(),
                    "missing {extension} on {} {method}",
                    operation.path
                );
            }
            assert!(operation_ids.insert(operation.operation_id));
            expected_operations.insert((operation.path, method));
        }

        let mut public_operations = std::collections::HashSet::new();
        for (path, path_item) in paths {
            for method in ["get", "post", "put", "patch", "delete"] {
                if path_item.get(method).is_some() {
                    public_operations.insert((path.as_str(), method));
                }
            }
        }
        assert_eq!(public_operations, expected_operations);

        let catalog_operation = &paths["/api/v1/catalog"]["get"];
        assert_eq!(catalog_operation["tags"], json!(["System · Katalog"]));
        for extension in [
            "x-nrg-domain",
            "x-nrg-roles",
            "x-nrg-sectors",
            "x-nrg-capabilities",
            "x-nrg-allocation-model",
            "x-nrg-generation-profiles",
        ] {
            assert!(
                !catalog_operation[extension].is_null(),
                "catalog operation is missing {extension}"
            );
        }

        let count_schema =
            &document["components"]["schemas"]["GenerateRequest"]["properties"]["count"];
        assert_eq!(count_schema["minimum"], 1);
        assert_eq!(count_schema["maximum"], 100);
        assert_eq!(
            document["x-nrg-catalog-version"],
            id_core::catalog::CATALOG_VERSION
        );
        assert_eq!(
            document["x-nrg-generator-version"],
            id_core::GENERATOR_VERSION
        );
        assert_eq!(
            document["x-nrg-reference-data"][0]["valid_to"],
            id_core::reference_data::BUNDESBANK_BLZ_VALID_TO
        );
        assert_eq!(
            document["x-nrg-reference-data"][1]["release"],
            id_core::identifiers::payments::international_iban::IBAN_REGISTRY_RELEASE
        );
        assert_eq!(
            document["x-nrg-reference-data"][2]["sha256"],
            id_core::reference_data::BDEW_IDENTIFIERS_METADATA.sha256
        );
        assert_eq!(
            document["x-nrg-reference-data"][3]["sha256"],
            id_core::reference_data::MASTR_PREFIXES_METADATA.sha256
        );
        assert_eq!(
            document["x-nrg-reference-data"][4]["record_count"],
            id_core::reference_data::ENTSO_E_EIC_DIRECTORY_RECORD_COUNT
        );
    }

    #[test]
    fn openapi_json_matches_reviewed_snapshot_hash() {
        fn canonicalize(value: Value) -> Value {
            match value {
                Value::Object(object) => {
                    let mut entries: Vec<_> = object.into_iter().collect();
                    entries.sort_by(|left, right| left.0.cmp(&right.0));
                    Value::Object(
                        entries
                            .into_iter()
                            .map(|(key, value)| (key, canonicalize(value)))
                            .collect(),
                    )
                }
                Value::Array(values) => {
                    Value::Array(values.into_iter().map(canonicalize).collect())
                }
                scalar => scalar,
            }
        }

        let document = serde_json::to_value(openapi()).unwrap();
        let snapshot = serde_json::to_vec(&canonicalize(document)).unwrap();
        let hash = snapshot.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });

        // Intentional OpenAPI contract changes require review and an explicit
        // update of both the byte count and hash.
        assert_eq!(snapshot.len(), 147_995);
        assert_eq!(hash, 14_670_226_568_695_231_399);
    }
}
