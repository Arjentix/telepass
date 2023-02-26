//! Module with `Database Service` implementation.

use std::ops::DerefMut;

use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use thiserror::Error;
use tonic::{Code, Request, Response, Status};
use tracing::{info, instrument};

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

impl From<Error> for ProcessingError {
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
        info!("Creating database connection pool...");
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = Pool::builder().build(manager)?;

        Ok(Self { pool })
    }

    /// Get database connection from the pool.
    fn connection(&self) -> Result<impl DerefMut<Target = PgConnection>> {
        self.pool.get().map_err(Into::into)
    }

    /// Call `f`, log the result and unpack `status` from [`ProcessingError`] if [`Err`].
    fn unpack_and_log<T: std::fmt::Debug>(
        f: impl FnOnce() -> Result<T, ProcessingError>,
    ) -> Result<T, Status> {
        match f() {
            Ok(response) => {
                tracing::info!(?response, "Request succeed");
                Ok(response)
            }
            Err(error) => {
                if error.status.code() == Code::Internal {
                    let additional_info = error.additional_info.unwrap_or_default();
                    tracing::error!(
                        %additional_info,
                        "Internal error occurred while processing request"
                    );
                } else {
                    let status = &error.status;
                    tracing::info!(?status, "Request failed");
                }

                Err(error.status)
            }
        }
    }
}

/// Internal error type for `gRPC` method implementations.
#[derive(Debug)]
struct ProcessingError {
    /// Status returned to the client.
    status: Status,
    /// Additional information about the error.
    additional_info: Option<String>,
}

impl ProcessingError {
    /// Construct new instance of [`ProcessingError`] with [`Status::internal()`] status.
    fn internal(additional_info: impl Into<String>) -> Self {
        Self {
            status: Status::internal("Internal error, please try again later"),
            additional_info: Some(additional_info.into()),
        }
    }
}

impl From<Status> for ProcessingError {
    fn from(status: Status) -> Self {
        if status.code() == Code::Internal {
            Self::internal(status.message())
        } else {
            Self {
                status,
                additional_info: None,
            }
        }
    }
}

#[tonic::async_trait]
impl grpc::database_service_server::DatabaseService for Database {
    #[instrument(skip(self))]
    async fn add(
        &self,
        request: Request<grpc::Record>,
    ) -> Result<Response<grpc::Response>, Status> {
        Self::unpack_and_log(|| {
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
                        Status::already_exists("Password for this resource already exists").into()
                    } else {
                        ProcessingError::internal(err.to_string())
                    }
                })?;

            Ok(Response::new(grpc::Response {}))
        })
    }

    #[instrument(skip(self))]
    async fn delete(
        &self,
        request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Response>, Status> {
        Self::unpack_and_log(|| {
            let resource_name = request.into_inner().name;

            let affected_rows =
                diesel::delete(passwords::table.filter(passwords::resource.eq(resource_name)))
                    .execute(&mut *self.connection()?)
                    .map_err(|err| ProcessingError::internal(err.to_string()))?;

            match affected_rows {
                0 => Err(Status::not_found("Resource not found").into()),
                1 => Ok(Response::new(grpc::Response {})),
                // Sanity check.
                _ => Err(ProcessingError::internal("More than one row affected")),
            }
        })
    }

    #[instrument(skip(self))]
    async fn get(
        &self,
        request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Record>, Status> {
        Self::unpack_and_log(|| {
            let resource_name = request.into_inner().name;

            passwords::table
                .filter(passwords::resource.eq(resource_name))
                .first::<models::Record>(&mut *self.connection()?)
                .map(|record| Response::new(grpc::Record::from(record)))
                .map_err(|err| {
                    if err == diesel::result::Error::NotFound {
                        Status::not_found("Resource not found").into()
                    } else {
                        ProcessingError::internal(err.to_string())
                    }
                })
        })
    }

    #[instrument(skip(self))]
    async fn list(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::ListOfRecords>, Status> {
        Self::unpack_and_log(|| {
            passwords::table
                .load::<models::Record>(&mut *self.connection()?)
                .map(|records| {
                    Response::new(grpc::ListOfRecords {
                        records: records.into_iter().map(grpc::Record::from).collect(),
                    })
                })
                .map_err(|err| ProcessingError::internal(err.to_string()))
        })
    }
}
