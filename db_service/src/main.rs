//! Telepass Database Service to store and retrieve passwords.

// Triggers on `base64` crate.
//
// TODO: Remove this once `tonic` uploads a new version to crates.io,
// because it's fixed on GitHub.
#![allow(clippy::multiple_crate_versions)]

use color_eyre::{eyre::WrapErr as _, Result};
use grpc::database_service_server::DatabaseServiceServer;
#[cfg(feature = "tls")]
use tonic::transport::{Certificate, Identity, ServerTlsConfig};
use tonic::{transport::Server, Request, Response, Status};
#[cfg(feature = "reflection")]
use tonic_reflection::server::{ServerReflection, ServerReflectionServer};

mod grpc {
    //! Module with generated `gRPC` code.

    #![allow(clippy::empty_structs_with_brackets)]
    #![allow(clippy::similar_names)]
    #![allow(clippy::default_trait_access)]
    #![allow(clippy::too_many_lines)]
    #![allow(clippy::clone_on_ref_ptr)]
    #![allow(clippy::shadow_unrelated)]
    #![allow(clippy::unwrap_used)]

    tonic::include_proto!("db_service");

    /// Descriptor used for reflection.
    #[cfg(feature = "reflection")]
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("db_service_descriptor");
}

/// Database service.
///
/// Handles client requests to store and retrieve passwords.
#[derive(Debug, Default)]
struct DatabaseService;

#[tonic::async_trait]
impl grpc::database_service_server::DatabaseService for DatabaseService {
    async fn add(
        &self,
        _request: Request<grpc::Record>,
    ) -> Result<Response<grpc::Response>, Status> {
        todo!()
    }

    async fn delete(
        &self,
        _request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Response>, Status> {
        todo!()
    }

    async fn get(
        &self,
        _request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Record>, Status> {
        todo!()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "[::1]:50051".parse()?;

    let db_service = DatabaseServiceServer::new(DatabaseService::default());

    #[allow(unused_mut)]
    let mut server = Server::builder();

    #[cfg(feature = "tls")]
    let mut server =
        server.tls_config(prepare_tls_config().wrap_err("Failed to prepare TLS configuration")?)?;

    let server = server.add_service(db_service);

    #[cfg(feature = "reflection")]
    let server = server
        .add_service(reflection_service().wrap_err("Failed to initialize reflection service")?);

    server.serve(addr).await.map_err(Into::into)
}

/// Prepares TLS configuration.
///
/// Reads server certificate, server private key and client authentication certificate
#[cfg(feature = "tls")]
fn prepare_tls_config() -> Result<ServerTlsConfig> {
    use std::{fs, path::PathBuf};

    let tls_dir = PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "..", "tls"]);

    let cert = fs::read_to_string(tls_dir.join("server.pem"))
        .wrap_err("Failed to read server certificate")?;
    let key = fs::read_to_string(tls_dir.join("server.key"))
        .wrap_err("Failed to read server private key")?;
    let server_identity = Identity::from_pem(cert, key);

    let client_ca_cert = std::fs::read_to_string(tls_dir.join("client.pem"))
        .wrap_err("Failed to read client authentication certificate")?;
    let client_ca_cert = Certificate::from_pem(client_ca_cert);

    Ok(ServerTlsConfig::new()
        .identity(server_identity)
        .client_ca_root(client_ca_cert))
}

/// Enable `gRPC` reflection.
#[cfg(feature = "reflection")]
fn reflection_service() -> Result<ServerReflectionServer<impl ServerReflection>> {
    tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(grpc::FILE_DESCRIPTOR_SET)
        .build()
        .map_err(Into::into)
}
