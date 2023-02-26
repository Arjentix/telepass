//! Module with `Database Service` implementation.

use std::ops::DerefMut;

use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use thiserror::Error;
use tonic::{Request, Response, Status};

use crate::{grpc, models, schema::passwords};

/// Result type for [`Database`] service.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error type for [`Database`] service.
#[derive(Debug, Error)]
pub enum Error {
    /// Error creating database connection pool.
    #[error("Failed to create connection pool: {0}")]
    FailedToCreateConnectionPool(#[from] diesel::r2d2::PoolError),
    /// Error getting database connection from the pool.
    #[error("Failed to get connection from the pool: {0}")]
    FailedToGetConnectionFromThePool(#[from] diesel::r2d2::Error),
}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        // Match is used to warn about new variants.
        match err {
            Error::FailedToCreateConnectionPool(_) | Error::FailedToGetConnectionFromThePool(_) => {
                Self::internal(err.to_string())
            }
        }
    }
}

/// Database service.
///
/// Handles client requests to store and retrieve passwords.
#[derive(Debug)]
pub struct Database {
    /// Database connection pool.
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl Database {
    /// Create new instance of [`Database`] service.
    ///
    /// # Errors
    ///
    /// Fails if failed to create database connection pool.
    pub fn new(database_url: &str) -> Result<Self> {
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = Pool::builder().build(manager)?;

        Ok(Self { pool })
    }

    /// Get database connection from the pool.
    fn connection(&self) -> Result<impl DerefMut<Target = PgConnection>> {
        self.pool.get().map_err(Into::into)
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
            .execute(&mut *self.connection()?)
            .map_err(|err| {
                if let diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ) = err
                {
                    Status::already_exists("Password for this resource already exists")
                } else {
                    Status::internal("Internal error, please try again later")
                }
            })?;

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
