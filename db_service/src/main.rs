//! Telepass Database Service to store and retrieve passwords.

// Triggers on `base64` crate.
//
// TODO: Remove this once `tonic` uploads a new version to crates.io,
// because it's fixed on GitHub.
#![allow(clippy::multiple_crate_versions)]

use std::{fs, path::PathBuf};

use color_eyre::{eyre::WrapErr as _, Result};
use grpc::database_service_server::DatabaseServiceServer;
use tonic::{
    transport::{Certificate, Identity, Server, ServerTlsConfig},
    Request, Response, Status,
};

pub mod grpc {
    //! Module with generated `gRPC` code.

    #![allow(clippy::empty_structs_with_brackets)]
    #![allow(clippy::similar_names)]
    #![allow(clippy::default_trait_access)]
    #![allow(clippy::too_many_lines)]
    #![allow(clippy::clone_on_ref_ptr)]
    #![allow(clippy::shadow_unrelated)]
    #![allow(clippy::unwrap_used)]

    tonic::include_proto!("db_service");
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
        request: Request<grpc::Record>,
    ) -> Result<Response<grpc::Response>, Status> {
        todo!()
    }

    async fn delete(
        &self,
        request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Response>, Status> {
        todo!()
    }

    async fn get(
        &self,
        request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Record>, Status> {
        todo!()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;

    let tls_dir = PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "..", "tls"]);
    let cert = fs::read_to_string(tls_dir.join("server.pem"))
        .wrap_err("Failed to read server certificate")?;
    let key = fs::read_to_string(tls_dir.join("server.key"))
        .wrap_err("Failed to read server private key")?;
    let server_identity = Identity::from_pem(cert, key);

    let client_ca_cert = std::fs::read_to_string(tls_dir.join("client.pem"))
        .wrap_err("Failed to read client authentication certificate")?;
    let client_ca_cert = Certificate::from_pem(client_ca_cert);

    let tls_config = ServerTlsConfig::new()
        .identity(server_identity)
        .client_ca_root(client_ca_cert);

    let db_service = DatabaseServiceServer::new(DatabaseService::default());

    Server::builder()
        .tls_config(tls_config)?
        .add_service(db_service)
        .serve(addr)
        .await?;

    Ok(())
}
