//! Module with `Database Service` implementation.

use diesel::{prelude::*, PgConnection};
use tonic::{Request, Response, Status};

use crate::{grpc, models, schema::passwords};

/// Database service.
///
/// Handles client requests to store and retrieve passwords.
#[derive(Debug, Default)]
pub struct Database;

impl Database {
    /// Get mutable connection to the database.
    fn connection_mut(&self) -> &mut PgConnection {
        todo!()
    }
}

#[tonic::async_trait]
impl grpc::database_service_server::DatabaseService for Database {
    async fn add(
        &self,
        request: Request<grpc::Record>,
    ) -> Result<Response<grpc::Response>, Status> {
        let raw_record = request.into_inner();
        let record = models::Record::try_from(raw_record)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        diesel::insert_into(passwords::table)
            .values(&record)
            .execute(self.connection_mut())
            .map_err(|err| Status::internal(err.to_string()))?;

        Ok(Response::new(grpc::Response {}))
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
