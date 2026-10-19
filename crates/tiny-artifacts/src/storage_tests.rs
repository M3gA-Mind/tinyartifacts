use super::*;

#[test]
fn declares_artifact_blob_prefix_for_tenant_scope() {
    let schema = storage_schema();
    let _registry = tinystoragedrivers_core::schema::Registry::compose([schema.clone()])
        .expect("artifact schema composes");

    assert_eq!(schema.owner.as_str(), "artifacts");
    assert_eq!(schema.items.len(), 1);
    assert_eq!(schema.items[0].name(), "artifacts/");
    assert_eq!(schema.items[0].scope(), ScopeClass::Tenant);
}
