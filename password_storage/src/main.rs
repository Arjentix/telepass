//! Telepass Password Storage Service to store and retrieve passwords.

#![cfg(feature = "executable")]

use color_eyre::{
    eyre::{eyre, WrapErr as _},
    Result,
};
use dotenvy::dotenv;
#[cfg(feature = "reflection")]
use telepass_password_storage::grpc;
use telepass_password_storage::{
    grpc::password_storage_server::PasswordStorageServer,
    service::{self},
};
use tonic::transport::Server;
#[cfg(feature = "tls")]
use tonic::transport::{Certificate, Identity, ServerTlsConfig};
#[cfg(feature = "reflection")]
use tonic_reflection::server::{ServerReflection, ServerReflectionServer};
use tracing::{info, Level};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().wrap_err("Failed to initialize logger")?;
    info!("Hello from Telepass Password Storage!");

    let rx = init_signal_handler()?;

    let _ignored = dotenv();

    let addr = "0.0.0.0:50051".parse()?;

    let database_url = read_env_var("DATABASE_URL")?;
    let cache_size = read_cache_size_env_var()?;
    let password_storage =
        PasswordStorageServer::new(service::PasswordStorage::new(&database_url, cache_size)?);

    #[expect(unused_mut, reason = "used in conditional compilation")]
    let mut server = Server::builder();

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<PasswordStorageServer<service::PasswordStorage>>()
        .await;

    #[cfg(feature = "tls")]
    let mut server = {
        let server = server
            .tls_config(prepare_tls_config().wrap_err("Failed to prepare TLS configuration")?)?;
        info!("TLS enabled");
        server
    };

    let server = server
        .add_service(health_service)
        .add_service(password_storage);

    #[cfg(feature = "reflection")]
    let server = {
        let server = server
            .add_service(reflection_service().wrap_err("Failed to initialize reflection service")?);
        info!("gRPC reflection enabled");
        server
    };

    info!("Listening on {}", addr);
    server.serve_with_shutdown(addr, recv_shutdown(rx)).await?;

    info!("Bye!");
    Ok(())
}

/// Initialize signal handler.
///
/// Returns a [`Receiver`](tokio::sync::oneshot::Receiver) that will
/// receive a shutdown message when program receive some kind of termination signal from OS.
#[expect(
    clippy::panic,
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "should never happen"
)]
fn init_signal_handler() -> Result<tokio::sync::oneshot::Receiver<()>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut tx_opt = Some(tx);
    ctrlc::set_handler(move || {
        info!("Received shutdown signal");
        tx_opt.take().map_or_else(
            || {
                panic!("Ctrl-C handler called twice");
            },
            |sender| {
                sender.send(()).expect("Shutdown signal receiver dropped");
            },
        );
    })
    .wrap_err("Failed to set Ctrl-C handler")?;
    Ok(rx)
}

/// Receive shutdown signal from `rx`.
///
/// Implemented as a function, because async closures are not yet supported.
#[expect(clippy::expect_used, reason = "should never happen")]
async fn recv_shutdown(rx: tokio::sync::oneshot::Receiver<()>) {
    rx.await.expect("Shutdown signal sender dropped");
}

/// Initialize logger.
fn init_logger() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber).wrap_err("Failed to set global logger")
}

/// Prepare TLS configuration.
///
/// Reads server certificate, server private key and client authentication certificate
#[cfg(feature = "tls")]
fn prepare_tls_config() -> Result<ServerTlsConfig> {
    let server_certificate_path = read_env_var("PASSWORD_STORAGE_TLS_CERT_PATH")?;
    let server_key_path = read_env_var("PASSWORD_STORAGE_TLS_KEY_PATH")?;
    let client_ca_cert_path = read_env_var("ROOT_CA_CERT_PATH")?;

    let cert = std::fs::read_to_string(&server_certificate_path).wrap_err_with(|| {
        format!("Failed to read server certificate at path: {server_certificate_path}",)
    })?;
    let key = std::fs::read_to_string(&server_key_path)
        .wrap_err_with(|| format!("Failed to read server key at path: {server_key_path}",))?;
    let server_identity = Identity::from_pem(cert, key);

    let client_ca_cert = std::fs::read_to_string(&client_ca_cert_path).wrap_err_with(|| {
        format!("Failed to read client certificate at path: {client_ca_cert_path}",)
    })?;
    let client_ca_cert = Certificate::from_pem(client_ca_cert);

    Ok(ServerTlsConfig::new()
        .identity(server_identity)
        .client_ca_root(client_ca_cert))
}

/// Read cache size from environment variable or use default value.
fn read_cache_size_env_var() -> Result<u32> {
    /// Environment variable to set cache size.
    const CACHE_SIZE_ENV_VAR: &str = "PASSWORD_STORAGE_CACHE_SIZE";
    /// Default cache size.
    const CACHE_SIZE_DEFAULT_VALUE: u32 = 1024;

    match std::env::var(CACHE_SIZE_ENV_VAR) {
        Ok(var) if var.is_empty() => {
            info!("`{CACHE_SIZE_ENV_VAR}` environment variable is empty. Using default value {CACHE_SIZE_DEFAULT_VALUE}");
            Ok(CACHE_SIZE_DEFAULT_VALUE)
        }
        Ok(var) => var.parse().wrap_err_with(|| {
            format!("Failed to parse `{CACHE_SIZE_ENV_VAR}` environment variable as integer",)
        }),
        Err(std::env::VarError::NotPresent) => {
            info!("`{CACHE_SIZE_ENV_VAR}` environment variable is not set. Using default value {CACHE_SIZE_DEFAULT_VALUE}");
            Ok(CACHE_SIZE_DEFAULT_VALUE)
        }
        Err(std::env::VarError::NotUnicode(_)) => Err(eyre!(
            "`{CACHE_SIZE_ENV_VAR}` environment variable is not in unicode format"
        )),
    }
}

/// Read `var` environment variable.
fn read_env_var(var: &str) -> Result<String> {
    std::env::var(var).wrap_err_with(|| format!("Expected `{var}` environment variable"))
}

/// Enable `gRPC` reflection.
#[cfg(feature = "reflection")]
fn reflection_service() -> Result<ServerReflectionServer<impl ServerReflection>> {
    tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(grpc::FILE_DESCRIPTOR_SET)
        .build_v1()
        .map_err(Into::into)
}
