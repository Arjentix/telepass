//! Module with generated `gRPC` code.

#![expect(
    clippy::empty_structs_with_brackets,
    clippy::similar_names,
    clippy::default_trait_access,
    clippy::too_many_lines,
    clippy::clone_on_ref_ptr,
    clippy::missing_const_for_fn,
    clippy::allow_attributes_without_reason,
    clippy::as_conversions,
    clippy::derive_partial_eq_without_eq,
    clippy::allow_attributes,
    reason = "generated code"
)]
#![allow(
    clippy::missing_docs_in_private_items,
    reason = "allow because it's false positive unfulfilled-lint-expectations if in expect"
)]

tonic::include_proto!("password_storage");

/// Descriptor used for reflection.
#[cfg(feature = "reflection")]
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("password_storage_descriptor");
