//! Module with [`PasswordStorage Service`](PasswordStorage) implementation.

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

mod cache;

/// Result type for [`PasswordStorage`] service.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Main error type for [`PasswordStorage`] service.
#[derive(Debug, Error)]
pub enum Error {
    /// Error creating database connection pool.
    #[error("Failed to create connection pool: {0}")]
    FailedToCreateConnectionPool(#[from] diesel::r2d2::PoolError),

    /// Error getting database connection from the pool.
    #[error("Failed to get connection from the pool: {0}")]
    FailedToGetConnectionFromThePool(#[from] diesel::r2d2::Error),

    /// Database error.
    ///
    /// Errors like [`NotFound`](diesel::result::Error::NotFound) and
    /// [`UniqueViolation`](diesel::result::DatabaseErrorKind::UniqueViolation)
    /// are represented by [`Error::NotFound`] and [`Error::AlreadyExists`].
    #[error("Database failure: {0}")]
    Database(diesel::result::Error),

    /// Invalid record.
    #[error("Invalid record: {0}")]
    InvalidRecord(#[from] models::ResourceIsMissingError),

    /// Record already exists.
    #[error("Password for resource `{0}` already exists")]
    AlreadyExists(String),

    /// Resource not found.
    #[error("Resource `{0}` not found")]
    NotFound(String),
}

/// Helper error type to wrap foreign errors with context.
struct ErrorWithContext<E> {
    /// Foreign error.
    internal: E,
    /// Resource name.
    resource_name: String,
}

/// Extension trait to easily construct [`ErrorWithContext`].
trait WithContextExt: Sized {
    /// Construct [`ErrorWithContext`] from `Self` and `resource_name`.
    fn with_context(self, resource_name: String) -> ErrorWithContext<Self>;
}

impl WithContextExt for diesel::result::Error {
    fn with_context(self, resource_name: String) -> ErrorWithContext<Self> {
        ErrorWithContext {
            internal: self,
            resource_name,
        }
    }
}

#[allow(clippy::wildcard_enum_match_arm)]
impl From<ErrorWithContext<diesel::result::Error>> for Error {
    fn from(error: ErrorWithContext<diesel::result::Error>) -> Self {
        match error.internal {
            diesel::result::Error::NotFound => Self::NotFound(error.resource_name),
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ) => Self::AlreadyExists(error.resource_name),
            internal => Self::Database(internal),
        }
    }
}

impl From<Error> for Status {
    fn from(error: Error) -> Self {
        match error {
            Error::FailedToCreateConnectionPool(_)
            | Error::FailedToGetConnectionFromThePool(_)
            | Error::Database(_) => Self::internal("Internal error, please try again later"),
            Error::InvalidRecord(_) => Self::invalid_argument(error.to_string()),
            Error::AlreadyExists(_) => Self::already_exists(error.to_string()),
            Error::NotFound(_) => Self::not_found(error.to_string()),
        }
    }
}

/// Password Storage service.
///
/// Handles client requests to store and retrieve passwords.
#[derive(Debug)]
pub struct PasswordStorage {
    /// PasswordStorage connection pool.
    pool: Pool<ConnectionManager<PgConnection>>,
    /// Cache for common requests.
    cache: cache::Cache,
}

impl PasswordStorage {
    /// Create new instance of [`PasswordStorage`] service.
    ///
    /// # Errors
    ///
    /// Fails if failed to create database connection pool.
    pub fn new(database_url: &str, cache_size: u32) -> Result<Self> {
        info!("Creating database connection pool...");
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = Pool::builder().build(manager)?;

        let cached_records = passwords::table
            .limit(cache_size.into())
            .load::<models::Record>(&mut pool.get()?)
            .map_err(Error::Database)?;

        let cache = cache::Cache::load(cache_size, cached_records);

        Ok(Self { pool, cache })
    }

    /// Get database connection from the pool.
    fn connection(&self) -> Result<impl DerefMut<Target = PgConnection>> {
        self.pool.get().map_err(Into::into)
    }

    /// Call `f`, log the result and unpack [`Status`] if [`Err`].
    fn log_and_transform<T: std::fmt::Debug>(f: impl FnOnce() -> Result<T>) -> Result<T, Status> {
        match f() {
            Ok(response) => {
                tracing::info!(?response, "Request succeed");
                Ok(response)
            }
            Err(error) => {
                let additional_info = error.to_string();
                let status = Status::from(error);
                if status.code() == Code::Internal {
                    tracing::error!(
                        %additional_info,
                        "Internal error occurred while processing request"
                    );
                } else {
                    tracing::info!(?status, "Request failed");
                }

                Err(status)
            }
        }
    }
}

#[tonic::async_trait]
impl grpc::password_storage_server::PasswordStorage for PasswordStorage {
    #[instrument(skip(self))]
    async fn add(
        &self,
        request: Request<grpc::Record>,
    ) -> Result<Response<grpc::Response>, Status> {
        Self::log_and_transform(|| {
            let raw_record = request.into_inner();
            let record = models::Record::try_from(raw_record)?;

            diesel::insert_into(passwords::table)
                .values(&record)
                .execute(&mut *self.connection()?)
                .map_err(|err| err.with_context(record.resource_name.clone()))?;
            self.cache.add(record);

            Ok(Response::new(grpc::Response {}))
        })
    }

    #[instrument(skip(self))]
    #[allow(clippy::panic)]
    async fn delete(
        &self,
        request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Response>, Status> {
        Self::log_and_transform(|| {
            let resource_name = request.into_inner().name;

            let affected_rows = diesel::delete(
                passwords::table.filter(passwords::resource_name.eq(&resource_name)),
            )
            .execute(&mut *self.connection()?)
            .map_err(|err| err.with_context(resource_name.clone()))?;

            match affected_rows {
                0 => Err(Error::NotFound(resource_name)),
                1 => {
                    self.cache.invalidate(&resource_name);
                    Ok(Response::new(grpc::Response {}))
                }
                n => panic!("More than one row affected while deleting record: {n} rows"),
            }
        })
    }

    #[instrument(skip(self))]
    async fn get(
        &self,
        request: Request<grpc::Resource>,
    ) -> Result<Response<grpc::Record>, Status> {
        Self::log_and_transform(|| {
            let resource_name = request.into_inner().name;

            self.cache
                .get_or_try_insert_with(&resource_name, || {
                    passwords::table
                        .filter(passwords::resource_name.eq(&resource_name))
                        .first::<models::Record>(&mut *self.connection()?)
                        .map_err(|err| err.with_context(resource_name.clone()))
                        .map_err(Into::into)
                })
                .map(|record| Response::new(grpc::Record::from(record)))
        })
    }

    #[instrument(skip(self))]
    async fn list(
        &self,
        _request: Request<grpc::Empty>,
    ) -> Result<Response<grpc::ListOfResources>, Status> {
        Self::log_and_transform(|| {
            let resource_names = self.cache.get_all_resources();

            Ok(Response::new(grpc::ListOfResources {
                resources: resource_names
                    .into_iter()
                    .map(|resource| grpc::Resource { name: resource })
                    .collect(),
            }))
        })
    }
}
