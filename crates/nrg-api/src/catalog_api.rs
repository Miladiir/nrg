//! Catalog HTTP representation and catalog-driven OpenAPI metadata.
//!
//! The transport-neutral descriptors live in `id_core`.  This module converts
//! them into owned DTOs so the HTTP contract never exposes static references,
//! and applies the same descriptors to the generated OpenAPI document.

use axum::Json;
use id_core::{
    catalog::{
        identifier_catalog, service_operations, ApiMethod, ApiOperationDescriptor,
        IdentifierDescriptor, ServiceOperationDescriptor, CATALOG_VERSION,
    },
    identifiers::payments::international_iban::{
        IBAN_REGISTRY_DATA_SHA256, IBAN_REGISTRY_NAME, IBAN_REGISTRY_PUBLISHED,
        IBAN_REGISTRY_RELEASE, IBAN_REGISTRY_SOURCE_URL,
    },
    reference_data::{
        BDEW_IDENTIFIERS_METADATA, BUNDESBANK_BLZ_NAME, BUNDESBANK_BLZ_SOURCE_SHA256,
        BUNDESBANK_BLZ_VALID_FROM, BUNDESBANK_BLZ_VALID_TO,
        ENTSO_E_EIC_DIRECTORY_ACTIVE_RECORD_COUNT, ENTSO_E_EIC_DIRECTORY_CREATED_AT,
        ENTSO_E_EIC_DIRECTORY_INACTIVE_RECORD_COUNT, ENTSO_E_EIC_DIRECTORY_NAME,
        ENTSO_E_EIC_DIRECTORY_PROJECTION_SHA256, ENTSO_E_EIC_DIRECTORY_RECORD_COUNT,
        ENTSO_E_EIC_DIRECTORY_SOURCE_SHA256, ENTSO_E_EIC_DIRECTORY_SOURCE_URL,
        MASTR_PREFIXES_METADATA,
    },
    GENERATOR_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{
    openapi::{extensions::Extensions, path::Operation, Deprecated, OpenApi},
    Modify, ToSchema,
};

/// Public, fully owned representation of the identifier catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CatalogResponse {
    /// Version of the catalog structure and descriptor set.
    pub catalog_version: String,
    /// Version of the deterministic generator algorithms.
    pub generator_version: String,
    /// Identifiers currently known to the service.
    pub identifiers: Vec<CatalogIdentifier>,
    /// Cross-identifier and system operations from the same central catalog.
    pub operations: Vec<CatalogOperation>,
}

impl CatalogResponse {
    /// Build a response from the central `id_core` catalog.
    pub fn from_identifier_catalog() -> Self {
        Self {
            catalog_version: CATALOG_VERSION.to_owned(),
            generator_version: GENERATOR_VERSION.to_owned(),
            identifiers: identifier_catalog()
                .iter()
                .map(CatalogIdentifier::from)
                .collect(),
            operations: service_operations()
                .iter()
                .map(|service| CatalogOperation::from(&service.operation))
                .collect(),
        }
    }
}

impl Default for CatalogResponse {
    fn default() -> Self {
        Self::from_identifier_catalog()
    }
}

/// Metadata for one identifier exposed by the public catalog endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CatalogIdentifier {
    pub kind: String,
    pub slug: String,
    pub label: String,
    pub description: String,
    pub format_description: String,
    pub examples: Vec<CatalogExample>,
    pub sources: Vec<CatalogSource>,
    pub domain: String,
    pub roles: Vec<String>,
    pub sectors: Vec<String>,
    pub capabilities: Vec<String>,
    pub checksum_scheme: Option<String>,
    pub allocation_model: String,
    pub generation_profiles: Vec<String>,
    pub default_profile: Option<String>,
    pub operations: Vec<CatalogOperation>,
}

impl From<&IdentifierDescriptor> for CatalogIdentifier {
    fn from(descriptor: &IdentifierDescriptor) -> Self {
        Self {
            kind: descriptor.kind.as_str().to_owned(),
            slug: descriptor.slug.to_owned(),
            label: descriptor.label.to_owned(),
            description: descriptor.description.to_owned(),
            format_description: descriptor.format_description.to_owned(),
            examples: descriptor
                .examples
                .iter()
                .map(|example| CatalogExample {
                    value: example.value.to_owned(),
                    label: example.label.to_owned(),
                })
                .collect(),
            sources: descriptor
                .sources
                .iter()
                .map(|source| CatalogSource {
                    label: source.label.to_owned(),
                    url: source.url.to_owned(),
                })
                .collect(),
            domain: descriptor.domain.as_str().to_owned(),
            roles: descriptor
                .roles
                .iter()
                .map(|role| role.as_str().to_owned())
                .collect(),
            sectors: descriptor
                .sectors
                .iter()
                .map(|sector| sector.as_str().to_owned())
                .collect(),
            capabilities: descriptor
                .capabilities
                .iter()
                .map(|capability| capability.as_str().to_owned())
                .collect(),
            checksum_scheme: descriptor
                .checksum_scheme
                .map(|scheme| scheme.as_str().to_owned()),
            allocation_model: descriptor.allocation_model.as_str().to_owned(),
            generation_profiles: descriptor
                .generation_profiles
                .iter()
                .map(|profile| profile.as_str().to_owned())
                .collect(),
            default_profile: descriptor
                .default_profile
                .map(|profile| profile.as_str().to_owned()),
            operations: descriptor
                .operations
                .iter()
                .map(CatalogOperation::from)
                .collect(),
        }
    }
}

/// Reviewed example attached to a catalog identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CatalogExample {
    pub value: String,
    pub label: String,
}

/// Authoritative documentation link attached to a catalog identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CatalogSource {
    pub label: String,
    pub url: String,
}

/// One explicit HTTP operation belonging to an identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CatalogOperation {
    pub path: String,
    /// Lowercase HTTP method, for example `get` or `post`.
    pub method: String,
    pub capability: String,
    pub operation_id: String,
    pub primary_tag: String,
    pub deprecated: bool,
}

impl From<&ApiOperationDescriptor> for CatalogOperation {
    fn from(operation: &ApiOperationDescriptor) -> Self {
        Self {
            path: operation.path.to_owned(),
            method: api_method_name(operation.method).to_owned(),
            capability: operation.capability.as_str().to_owned(),
            operation_id: operation.operation_id.to_owned(),
            primary_tag: operation.primary_tag.to_owned(),
            deprecated: operation.deprecated,
        }
    }
}

/// Return the central identifier catalog used by routes, OpenAPI and clients.
#[utoipa::path(
    get,
    path = "/api/v1/catalog",
    operation_id = "systemCatalogList",
    responses(
        (status = 200, description = "Identifier catalog and supported operations", body = CatalogResponse)
    ),
    tag = "System · Katalog"
)]
pub async fn handle_catalog() -> Json<CatalogResponse> {
    Json(CatalogResponse::from_identifier_catalog())
}

/// A catalog operation for which no matching OpenAPI path and method exists.
///
/// The modifier deliberately ignores these at runtime.  Consistency tests can
/// use [`missing_catalog_operations`] to turn incomplete route documentation
/// into an actionable test failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub struct MissingCatalogOperation {
    pub identifier_slug: String,
    pub path: String,
    pub method: String,
    pub operation_id: String,
}

/// Return catalog operations absent from an OpenAPI document.
#[cfg(test)]
pub fn missing_catalog_operations(openapi: &OpenApi) -> Vec<MissingCatalogOperation> {
    let mut missing: Vec<_> = identifier_catalog()
        .iter()
        .flat_map(|descriptor| {
            descriptor
                .operations
                .iter()
                .filter(|catalog_operation| openapi_operation(openapi, catalog_operation).is_none())
                .map(|catalog_operation| MissingCatalogOperation {
                    identifier_slug: descriptor.slug.to_owned(),
                    path: catalog_operation.path.to_owned(),
                    method: api_method_name(catalog_operation.method).to_owned(),
                    operation_id: catalog_operation.operation_id.to_owned(),
                })
        })
        .collect();
    missing.extend(service_operations().iter().filter_map(|service| {
        let operation = &service.operation;
        openapi_operation(openapi, operation)
            .is_none()
            .then(|| MissingCatalogOperation {
                identifier_slug: "service".to_owned(),
                path: operation.path.to_owned(),
                method: api_method_name(operation.method).to_owned(),
                operation_id: operation.operation_id.to_owned(),
            })
    }));
    missing
}

/// Applies the central catalog metadata to operations already present in OAS.
///
/// Missing operations are left untouched and can be diagnosed with
/// [`missing_catalog_operations`].  Keeping mutation best-effort avoids a
/// runtime panic while an endpoint and its OpenAPI registration are being
/// integrated in separate steps.
pub struct CatalogOpenApiModifier;

impl Modify for CatalogOpenApiModifier {
    fn modify(&self, openapi: &mut OpenApi) {
        for descriptor in identifier_catalog() {
            for catalog_operation in descriptor.operations {
                if let Some(operation) = openapi_operation_mut(openapi, catalog_operation) {
                    apply_catalog_metadata(operation, descriptor, catalog_operation);
                }
            }
        }
        for service in service_operations() {
            if let Some(operation) = openapi_operation_mut(openapi, &service.operation) {
                apply_service_metadata(operation, service);
            }
        }
        apply_document_metadata(openapi);
    }
}

fn apply_document_metadata(openapi: &mut OpenApi) {
    openapi.info.description = Some(format!(
        "Generate, validate, parse and look up energy-market, payment, business-register and metering identifiers. Embedded Bundesbank BLZ reference data is valid from {BUNDESBANK_BLZ_VALID_FROM} through {BUNDESBANK_BLZ_VALID_TO}; international IBAN formats use SWIFT registry release {IBAN_REGISTRY_RELEASE}. Versioned BDEW and MaStR rule catalogs and an embedded ENTSO-E EIC directory snapshot document the exact offline evidence used. Format, checksum or directory evidence never proves account existence or real-world assignment."
    ));
    let extensions = openapi.extensions.get_or_insert_with(Extensions::default);
    extensions.insert(
        "x-nrg-catalog-version".to_string(),
        Value::String(CATALOG_VERSION.to_string()),
    );
    extensions.insert(
        "x-nrg-generator-version".to_string(),
        Value::String(GENERATOR_VERSION.to_string()),
    );
    extensions.insert(
        "x-nrg-reference-data".to_string(),
        serde_json::json!([
            {
                "name": BUNDESBANK_BLZ_NAME,
                "valid_from": BUNDESBANK_BLZ_VALID_FROM,
                "valid_to": BUNDESBANK_BLZ_VALID_TO,
                "sha256": BUNDESBANK_BLZ_SOURCE_SHA256,
            },
            {
                "name": IBAN_REGISTRY_NAME,
                "release": IBAN_REGISTRY_RELEASE,
                "published": IBAN_REGISTRY_PUBLISHED,
                "source_url": IBAN_REGISTRY_SOURCE_URL,
                "sha256": IBAN_REGISTRY_DATA_SHA256,
            },
            {
                "name": BDEW_IDENTIFIERS_METADATA.name,
                "version": BDEW_IDENTIFIERS_METADATA.version,
                "published": BDEW_IDENTIFIERS_METADATA.published,
                "checked_at": BDEW_IDENTIFIERS_METADATA.checked_at,
                "source_url": BDEW_IDENTIFIERS_METADATA.source_url,
                "sha256": BDEW_IDENTIFIERS_METADATA.sha256,
                "source_sha256": BDEW_IDENTIFIERS_METADATA.source_sha256,
                "contains_allocations": BDEW_IDENTIFIERS_METADATA.contains_allocations,
            },
            {
                "name": MASTR_PREFIXES_METADATA.name,
                "version": MASTR_PREFIXES_METADATA.version,
                "published": MASTR_PREFIXES_METADATA.published,
                "checked_at": MASTR_PREFIXES_METADATA.checked_at,
                "source_url": MASTR_PREFIXES_METADATA.source_url,
                "sha256": MASTR_PREFIXES_METADATA.sha256,
                "source_sha256": MASTR_PREFIXES_METADATA.source_sha256,
                "contains_allocations": MASTR_PREFIXES_METADATA.contains_allocations,
            },
            {
                "name": ENTSO_E_EIC_DIRECTORY_NAME,
                "created_at": ENTSO_E_EIC_DIRECTORY_CREATED_AT,
                "source_url": ENTSO_E_EIC_DIRECTORY_SOURCE_URL,
                "sha256": ENTSO_E_EIC_DIRECTORY_PROJECTION_SHA256,
                "source_sha256": ENTSO_E_EIC_DIRECTORY_SOURCE_SHA256,
                "record_count": ENTSO_E_EIC_DIRECTORY_RECORD_COUNT,
                "active_record_count": ENTSO_E_EIC_DIRECTORY_ACTIVE_RECORD_COUNT,
                "inactive_record_count": ENTSO_E_EIC_DIRECTORY_INACTIVE_RECORD_COUNT,
            }
        ]),
    );
}

fn apply_service_metadata(operation: &mut Operation, service: &ServiceOperationDescriptor) {
    let catalog_operation = &service.operation;
    operation.tags = Some(vec![catalog_operation.primary_tag.to_owned()]);
    operation.operation_id = Some(catalog_operation.operation_id.to_owned());
    operation.deprecated = Some(if catalog_operation.deprecated {
        Deprecated::True
    } else {
        Deprecated::False
    });

    let extensions = operation.extensions.get_or_insert_with(Extensions::default);
    extensions.insert(
        "x-nrg-domain".to_owned(),
        Value::String(service.domain.as_str().to_owned()),
    );
    extensions.insert(
        "x-nrg-roles".to_owned(),
        string_array(service.roles.iter().map(|role| role.as_str())),
    );
    extensions.insert(
        "x-nrg-sectors".to_owned(),
        string_array(service.sectors.iter().map(|sector| sector.as_str())),
    );
    extensions.insert(
        "x-nrg-capabilities".to_owned(),
        string_array([catalog_operation.capability.as_str()]),
    );
    extensions.insert(
        "x-nrg-allocation-model".to_owned(),
        Value::String(service.allocation_model.as_str().to_owned()),
    );
    extensions.insert(
        "x-nrg-generation-profiles".to_owned(),
        string_array(
            service
                .generation_profiles
                .iter()
                .map(|profile| profile.as_str()),
        ),
    );
}

fn apply_catalog_metadata(
    operation: &mut Operation,
    descriptor: &IdentifierDescriptor,
    catalog_operation: &ApiOperationDescriptor,
) {
    // Roles and sectors are facets, not normal tags.  Replacing the complete
    // list guarantees that every catalog operation has exactly one primary tag.
    operation.tags = Some(vec![catalog_operation.primary_tag.to_owned()]);
    operation.operation_id = Some(catalog_operation.operation_id.to_owned());
    operation.deprecated = Some(if catalog_operation.deprecated {
        Deprecated::True
    } else {
        Deprecated::False
    });

    let extensions = operation.extensions.get_or_insert_with(Extensions::default);
    extensions.insert(
        "x-nrg-domain".to_owned(),
        Value::String(descriptor.domain.as_str().to_owned()),
    );
    extensions.insert(
        "x-nrg-roles".to_owned(),
        string_array(descriptor.roles.iter().map(|role| role.as_str())),
    );
    extensions.insert(
        "x-nrg-sectors".to_owned(),
        string_array(descriptor.sectors.iter().map(|sector| sector.as_str())),
    );
    extensions.insert(
        "x-nrg-capabilities".to_owned(),
        string_array(
            descriptor
                .capabilities
                .iter()
                .map(|capability| capability.as_str()),
        ),
    );
    extensions.insert(
        "x-nrg-allocation-model".to_owned(),
        Value::String(descriptor.allocation_model.as_str().to_owned()),
    );
    extensions.insert(
        "x-nrg-generation-profiles".to_owned(),
        string_array(
            descriptor
                .generation_profiles
                .iter()
                .map(|profile| profile.as_str()),
        ),
    );
    extensions.insert(
        "x-nrg-format".to_owned(),
        Value::String(descriptor.format_description.to_owned()),
    );
    extensions.insert(
        "x-nrg-sources".to_owned(),
        Value::Array(
            descriptor
                .sources
                .iter()
                .map(|source| {
                    serde_json::json!({
                        "label": source.label,
                        "url": source.url,
                    })
                })
                .collect(),
        ),
    );
}

fn string_array<'a>(values: impl IntoIterator<Item = &'a str>) -> Value {
    Value::Array(
        values
            .into_iter()
            .map(|value| Value::String(value.to_owned()))
            .collect(),
    )
}

fn api_method_name(method: ApiMethod) -> &'static str {
    method.as_str()
}

#[cfg(test)]
fn openapi_operation<'a>(
    openapi: &'a OpenApi,
    catalog_operation: &ApiOperationDescriptor,
) -> Option<&'a Operation> {
    let path_item = openapi.paths.paths.get(catalog_operation.path)?;
    match catalog_operation.method {
        ApiMethod::Get => path_item.get.as_ref(),
        ApiMethod::Post => path_item.post.as_ref(),
    }
}

fn openapi_operation_mut<'a>(
    openapi: &'a mut OpenApi,
    catalog_operation: &ApiOperationDescriptor,
) -> Option<&'a mut Operation> {
    let path_item = openapi.paths.paths.get_mut(catalog_operation.path)?;
    match catalog_operation.method {
        ApiMethod::Get => path_item.get.as_mut(),
        ApiMethod::Post => path_item.post.as_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use utoipa::openapi::{
        extensions::ExtensionsBuilder,
        path::{HttpMethod, OperationBuilder},
        Info, Paths,
    };

    fn utoipa_method(method: ApiMethod) -> HttpMethod {
        match method {
            ApiMethod::Get => HttpMethod::Get,
            ApiMethod::Post => HttpMethod::Post,
        }
    }

    fn document_with(operation: &ApiOperationDescriptor) -> OpenApi {
        let operation_model = OperationBuilder::new()
            .tags(Some(["obsolete tag"]))
            .operation_id(Some("obsoleteOperationId"))
            .extensions(Some(
                ExtensionsBuilder::new()
                    .add("x-existing", "preserved")
                    .build(),
            ))
            .build();
        let mut paths = Paths::new();
        paths.add_path_operation(
            operation.path,
            vec![utoipa_method(operation.method)],
            operation_model,
        );
        OpenApi::new(Info::new("test", "1"), paths)
    }

    #[test]
    fn catalog_dto_is_owned_complete_and_serializable() {
        let response = CatalogResponse::from_identifier_catalog();

        assert_eq!(response.catalog_version, CATALOG_VERSION);
        assert_eq!(response.generator_version, GENERATOR_VERSION);
        assert_eq!(response.identifiers.len(), identifier_catalog().len());
        assert_eq!(response.operations.len(), service_operations().len());

        let core = &identifier_catalog()[0];
        let dto = &response.identifiers[0];
        assert_eq!(dto.kind, core.kind.as_str());
        assert_eq!(dto.slug, core.slug);
        assert_eq!(dto.description, core.description);
        assert_eq!(dto.format_description, core.format_description);
        assert_eq!(dto.examples.len(), core.examples.len());
        assert_eq!(dto.sources.len(), core.sources.len());
        assert_eq!(dto.domain, core.domain.as_str());
        assert_eq!(dto.operations.len(), core.operations.len());

        let serialized = serde_json::to_value(&response).expect("catalog must serialize");
        assert_eq!(serialized["catalog_version"], json!(CATALOG_VERSION));
        assert_eq!(serialized["generator_version"], json!(GENERATOR_VERSION));
        assert!(serialized["identifiers"].is_array());
        assert!(serialized["operations"].is_array());
    }

    #[test]
    fn catalog_handler_has_stable_openapi_identity() {
        #[derive(utoipa::OpenApi)]
        #[openapi(
            paths(handle_catalog),
            components(schemas(
                CatalogResponse,
                CatalogIdentifier,
                CatalogExample,
                CatalogSource,
                CatalogOperation
            ))
        )]
        struct CatalogDoc;

        let document = <CatalogDoc as utoipa::OpenApi>::openapi();
        let operation = document
            .paths
            .paths
            .get("/api/v1/catalog")
            .and_then(|path| path.get.as_ref())
            .expect("catalog GET operation");

        assert_eq!(operation.operation_id.as_deref(), Some("systemCatalogList"));
        assert_eq!(
            operation.tags.as_deref(),
            Some(&["System · Katalog".to_owned()][..])
        );
    }

    #[test]
    fn modifier_replaces_primary_metadata_and_preserves_other_extensions() {
        let descriptor = &identifier_catalog()[0];
        let catalog_operation = &descriptor.operations[0];
        let mut openapi = document_with(catalog_operation);

        CatalogOpenApiModifier.modify(&mut openapi);

        let operation = openapi_operation(&openapi, catalog_operation)
            .expect("fixture operation must remain present");
        assert_eq!(
            operation.tags.as_deref(),
            Some(&[catalog_operation.primary_tag.to_owned()][..])
        );
        assert_eq!(
            operation.operation_id.as_deref(),
            Some(catalog_operation.operation_id)
        );
        assert!(matches!(operation.deprecated, Some(Deprecated::True)));

        let extensions = operation.extensions.as_ref().expect("extensions");
        assert_eq!(extensions.get("x-existing"), Some(&json!("preserved")));
        assert_eq!(
            extensions.get("x-nrg-domain"),
            Some(&json!(descriptor.domain.as_str()))
        );
        assert_eq!(
            extensions.get("x-nrg-roles"),
            Some(&string_array(
                descriptor.roles.iter().map(|role| role.as_str())
            ))
        );
        assert_eq!(
            extensions.get("x-nrg-sectors"),
            Some(&string_array(
                descriptor.sectors.iter().map(|sector| sector.as_str())
            ))
        );
        assert_eq!(
            extensions.get("x-nrg-capabilities"),
            Some(&string_array(
                descriptor
                    .capabilities
                    .iter()
                    .map(|capability| capability.as_str())
            ))
        );
        assert_eq!(
            extensions.get("x-nrg-allocation-model"),
            Some(&json!(descriptor.allocation_model.as_str()))
        );
        assert_eq!(
            extensions.get("x-nrg-generation-profiles"),
            Some(&string_array(
                descriptor
                    .generation_profiles
                    .iter()
                    .map(|profile| profile.as_str())
            ))
        );
        assert_eq!(
            extensions.get("x-nrg-format"),
            Some(&json!(descriptor.format_description))
        );
        assert_eq!(
            extensions.get("x-nrg-sources"),
            Some(&json!(descriptor
                .sources
                .iter()
                .map(|source| json!({ "label": source.label, "url": source.url }))
                .collect::<Vec<_>>()))
        );
    }

    #[test]
    fn missing_operations_are_reported_and_do_not_make_modifier_panic() {
        let mut openapi = OpenApi::new(Info::new("test", "1"), Paths::new());
        let expected_count: usize = identifier_catalog()
            .iter()
            .map(|descriptor| descriptor.operations.len())
            .sum::<usize>()
            + service_operations().len();

        CatalogOpenApiModifier.modify(&mut openapi);

        let missing = missing_catalog_operations(&openapi);
        assert_eq!(missing.len(), expected_count);
        assert!(missing.iter().all(|operation| {
            !operation.identifier_slug.is_empty()
                && !operation.path.is_empty()
                && !operation.method.is_empty()
                && !operation.operation_id.is_empty()
        }));
    }

    #[test]
    fn present_operation_is_not_reported_missing() {
        let catalog_operation = &identifier_catalog()[0].operations[0];
        let openapi = document_with(catalog_operation);

        let missing = missing_catalog_operations(&openapi);

        assert!(!missing
            .iter()
            .any(|operation| operation.operation_id == catalog_operation.operation_id));
    }
}
