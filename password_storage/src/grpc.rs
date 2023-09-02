//! Module with generated `gRPC` code.

#![allow(clippy::empty_structs_with_brackets)]
#![allow(clippy::similar_names)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::clone_on_ref_ptr)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::missing_docs_in_private_items)]

tonic::include_proto!("password_storage");

/// Descriptor used for reflection.
#[cfg(feature = "reflection")]
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("password_storage_descriptor");
