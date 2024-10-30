//! Module with generated `gRPC` code.

#![allow(
    clippy::empty_structs_with_brackets,
    clippy::similar_names,
    clippy::default_trait_access,
    clippy::too_many_lines,
    clippy::clone_on_ref_ptr,
    clippy::shadow_unrelated,
    clippy::unwrap_used,
    clippy::missing_const_for_fn,
    clippy::missing_docs_in_private_items,
    clippy::allow_attributes_without_reason,
    clippy::as_conversions,
    clippy::derive_partial_eq_without_eq,
    reason = "generated code"
)]

tonic::include_proto!("password_storage");

/// Descriptor used for reflection.
#[cfg(feature = "reflection")]
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("password_storage_descriptor");
