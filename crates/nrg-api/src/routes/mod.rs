use axum::{extract::rejection::JsonRejection, Json};
use id_core::ops::{self, GenerateOptions, OpsError};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{ApiError, ErrorResponse};

pub(crate) mod business;
pub(crate) mod energy;
pub(crate) mod metering;
pub(crate) mod payments;
pub(crate) mod registers;

/// Options accepted by every generator endpoint.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(default)]
pub(crate) struct GenerateRequest {
    /// Number of values to generate (default 1).
    #[schema(minimum = 1, maximum = 100)]
    pub count: Option<u32>,
    /// Reproducible seed: the same seed reproduces the same values. Omitted
    /// means a random seed.
    pub seed: Option<String>,
}

impl GenerateRequest {
    pub(crate) fn into_options(self) -> GenerateOptions {
        GenerateOptions {
            count: self.count,
            seed: self.seed,
            ..GenerateOptions::default()
        }
    }
}

pub(crate) type GeneratePayload = Result<Json<GenerateRequest>, JsonRejection>;

/// One batch of generated test values.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct GenerateResponse {
    pub values: Vec<String>,
}

/// Result of validating one identifier value.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ValidateResponse {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Market sector selecting sector-specific formation rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Sector {
    Electricity,
    Gas,
}

impl Sector {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Electricity => "electricity",
            Self::Gas => "gas",
        }
    }
}

/// Output representation for identifiers with a formatted form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Format {
    Electronic,
    Formatted,
}

impl Format {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Electronic => "electronic",
            Self::Formatted => "formatted",
        }
    }
}

// Documentation-only enums; they are never instantiated at runtime.
#[allow(dead_code)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum GenerateApiResponses {
    /// Generated value batch.
    #[response(status = 200)]
    Success(GenerateResponse),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// The request schema or generation options are invalid.
    #[response(status = 422)]
    InvalidOptions(ErrorResponse),
    /// Generation failed an internal invariant.
    #[response(status = 500)]
    GenerationFailed(ErrorResponse),
}

#[allow(dead_code)]
#[derive(utoipa::IntoResponses)]
pub(crate) enum ValidateApiResponses {
    /// Validation result.
    #[response(status = 200)]
    Success(ValidateResponse),
    /// Malformed JSON request body.
    #[response(status = 400)]
    MalformedJson(ErrorResponse),
    /// Request body exceeds the configured limit.
    #[response(status = 413)]
    PayloadTooLarge(ErrorResponse),
    /// Content-Type is not application/json.
    #[response(status = 415)]
    UnsupportedMediaType(ErrorResponse),
    /// The request does not match the schema.
    #[response(status = 422)]
    InvalidRequest(ErrorResponse),
}

/// Runs the shared `id_core::ops` generator dispatch for one endpoint.
pub(crate) fn run_generate(
    slug: &str,
    options: GenerateOptions,
) -> Result<Json<GenerateResponse>, ApiError> {
    match ops::generate(slug, &options) {
        Ok(values) => Ok(Json(GenerateResponse { values })),
        Err(OpsError::InvalidOptions(message)) => Err(ApiError::invalid_request(message)),
        Err(OpsError::Failed(message)) => {
            Err(ApiError::generation_failed_with_message(slug, message))
        }
    }
}

pub(crate) fn generate_batch(
    slug: &str,
    payload: GeneratePayload,
) -> Result<Json<GenerateResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::invalid_generate_json)?;
    run_generate(slug, request.into_options())
}

pub(crate) fn validation_response(result: Result<(), String>) -> Json<ValidateResponse> {
    Json(match result {
        Ok(()) => ValidateResponse {
            valid: true,
            error: None,
        },
        Err(error) => ValidateResponse {
            valid: false,
            error: Some(error),
        },
    })
}
