//! Data structures to be passed to/from database

use diesel::prelude::*;
use thiserror::Error;

use crate::schema::passwords;

/// `passwords` database record
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Insertable)]
#[diesel(table_name = passwords)]
pub struct Record {
    /// Name of the resource
    pub resource: String,
    /// Hashed password
    pub passhash: String,
    /// Salt applied to the hashed password
    pub salt: String,
}

/// Error indicating that `resource` field is missing
#[derive(Debug, Copy, Clone, Error)]
#[error("`resource` is missing")]
pub struct ResourceIsMissingError;

impl TryFrom<crate::grpc::Record> for Record {
    type Error = ResourceIsMissingError;

    fn try_from(value: crate::grpc::Record) -> Result<Self, Self::Error> {
        Ok(Self {
            resource: value.resource.ok_or(ResourceIsMissingError)?.name,
            passhash: value.passhash,
            salt: value.salt,
        })
    }
}

impl From<Record> for crate::grpc::Record {
    fn from(value: Record) -> Self {
        Self {
            resource: Some(crate::grpc::Resource {
                name: value.resource,
            }),
            passhash: value.passhash,
            salt: value.salt,
        }
    }
}
