//! Build script to build the `gRPC` service.

use color_eyre::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .build_server(false)
        .compile(&["../proto/password_storage.proto"], &["../proto"])
        .map_err(Into::into)
}
