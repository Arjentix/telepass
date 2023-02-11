//! Telepass Database Service to store and retrieve passwords.

use grpc::database_service_server::DatabaseServiceServer;
use tonic::{transport::Server, Request, Response, Status};

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
    let service = DatabaseService::default();

    Server::builder()
        .add_service(DatabaseServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
