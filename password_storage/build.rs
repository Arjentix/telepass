//! Build script to build the `gRPC` service.

use std::{env, path::PathBuf};

use color_eyre::{eyre::WrapErr as _, Result};

fn main() -> Result<()> {
    let out_dir = env::var("OUT_DIR").wrap_err("Expected `OUT_DIR` env var")?;
    let descriptor_path = PathBuf::from(out_dir).join("password_storage_descriptor.bin");

    tonic_build::configure()
        .file_descriptor_set_path(descriptor_path)
        .build_client(false)
        .compile(&["../proto/password_storage.proto"], &["../proto"])
        .map_err(Into::into)
}
