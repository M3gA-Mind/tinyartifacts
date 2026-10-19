//! Tiny Storage Drivers declarations owned by this crate.

use tinystoragedrivers_core::schema::{Owner, Schema, ScopeClass};

/// Declare the blob namespace used by [`crate::BlobFileStore`].
///
/// Hosts compose this with their other storage schemas so the driver can
/// reserve the `artifacts/` prefix in the same scope used for artifact data.
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "the constant owner name is valid and this declaration is tested"
)]
pub fn storage_schema() -> Schema {
    Schema::new(Owner::new("artifacts").expect("`artifacts` is a valid owner"))
        .blobs("artifacts/", ScopeClass::Tenant)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "storage_tests.rs"]
mod tests;
