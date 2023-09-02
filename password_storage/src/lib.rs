//! Telepass Password Storage Service library to store and retrieve passwords.

pub mod grpc;
pub mod models;
/// Module with database schema generated by `diesel`
#[allow(clippy::single_char_lifetime_names)]
pub mod schema;
pub mod service;
