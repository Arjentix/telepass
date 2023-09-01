//! Build script to build the `gRPC` service.

use color_eyre::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .build_server(false)
        .client_mod_attribute(
            "password_storage",
            "#[allow(clippy::missing_docs_in_private_items)]",
        )
        .compile(&["../proto/password_storage.proto"], &["../proto"])
        .map_err(Into::into)
}
