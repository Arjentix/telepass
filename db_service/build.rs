//! Build script to build the `gRPC` service.

use color_eyre::Result;

fn main() -> Result<()> {
    tonic_build::configure()
        .build_client(false)
        .compile(&["../proto/db_service.proto"], &["../proto"])
        .map_err(Into::into)
}
