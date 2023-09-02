//! Telepass Password Storage Service to store and retrieve passwords.

use std::env;

use color_eyre::{eyre::WrapErr as _, Result};
use dotenvy::dotenv;
#[cfg(feature = "reflection")]
use telepass_password_storage::grpc;
use telepass_password_storage::{grpc::password_storage_server::PasswordStorageServer, service};
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
    info!("Hello from Telepass PasswordStorage Service!");

    let rx = init_signal_handler()?;

    dotenv().wrap_err("Failed to load `.env` file")?;

    let addr = "0.0.0.0:50051".parse()?;

    let database_url = env::var("DATABASE_URL").wrap_err("`DATABASE_URL` must be set")?;
    let cache_size = env::var("CACHE_SIZE")
        .wrap_err("`CACHE_SIZE` must be set")
        .map(|s| s.parse().wrap_err("Failed to parse `CACHE_SIZE`"))??;
    let password_storage =
        PasswordStorageServer::new(service::PasswordStorage::new(&database_url, cache_size)?);

    #[allow(unused_mut)]
    let mut server = Server::builder();

    #[cfg(feature = "tls")]
    let mut server = {
        let server = server
            .tls_config(prepare_tls_config().wrap_err("Failed to prepare TLS configuration")?)?;
        info!("TLS enabled");
        server
    };

    let server = server.add_service(password_storage);

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
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
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
#[allow(clippy::expect_used)]
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
    use std::{fs, path::PathBuf};

    let certs_dir = PathBuf::from_iter([std::env!("CARGO_MANIFEST_DIR"), "..", "certs"]);
    let server_certificate_path = certs_dir.join("password_storage.crt");
    let server_key_path = certs_dir.join("password_storage.key");
    let client_ca_cert_path = certs_dir.join("root_ca.crt");

    let cert = fs::read_to_string(&server_certificate_path).wrap_err_with(|| {
        format!(
            "Failed to read server certificate at path: {}",
            server_certificate_path.display()
        )
    })?;
    let key = fs::read_to_string(&server_key_path).wrap_err_with(|| {
        format!(
            "Failed to read server private key at path: {}",
            server_key_path.display()
        )
    })?;
    let server_identity = Identity::from_pem(cert, key);

    let client_ca_cert = std::fs::read_to_string(&client_ca_cert_path).wrap_err_with(|| {
        format!(
            "Failed to read client certificate at path: {}",
            client_ca_cert_path.display()
        )
    })?;
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
