//! Build script to build the `gRPC` service.

use color_eyre::Result;

fn main() -> Result<()> {
    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .client_mod_attribute(
            "password_storage",
            "#[expect(clippy::missing_docs_in_private_items)]",
        )
        .compile_protos(&["../proto/password_storage.proto"], &["../proto"])
        .map_err(Into::into)
}
