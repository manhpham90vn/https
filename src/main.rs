use https_proxy::config::Config;
use https_proxy::proxy_handler;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{routing::any, Router};
use axum_server::tls_rustls::RustlsConfig;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_CERT_PATH: &str = "/certs/cert.pem";
const DEFAULT_KEY_PATH: &str = "/certs/key.pem";
const DEFAULT_CONFIG_PATH: &str = "/etc/proxy/routes.yaml";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install default crypto provider (required for rustls 0.23+)
    // Ignore error if already installed
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "https_proxy=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config_path =
        std::env::var("CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
    tracing::info!("Loading config from: {}", config_path);
    let config = Config::load(&config_path)?;
    tracing::info!("Loaded {} listeners", config.listeners.len());
    for listener in &config.listeners {
        tracing::info!("  :{} -> {}", listener.port, listener.target);
    }

    // Create insecure TLS config for upstream connections
    // We create Two copies: one for hyper-rustls (it consumes it) and one for tungstenite (shared via Arc)
    let https_client_config = https_proxy::tls::get_insecure_client_config();
    let ws_client_config = Arc::new(https_proxy::tls::get_insecure_client_config());

    // Create HTTPS-capable client for proxying (supports both HTTP and HTTPS upstream)
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(https_client_config)
        .https_or_http()
        .enable_http1()
        .build();
    let http_client = Arc::new(Client::builder(TokioExecutor::new()).build(https));

    // Load TLS configuration (shared across all listeners)
    let cert_path = std::env::var("CERT_PATH").unwrap_or_else(|_| DEFAULT_CERT_PATH.to_string());
    let key_path = std::env::var("KEY_PATH").unwrap_or_else(|_| DEFAULT_KEY_PATH.to_string());

    tracing::info!("Loading TLS cert from: {}", cert_path);
    tracing::info!("Loading TLS key from: {}", key_path);

    let rustls_config = RustlsConfig::from_pem_file(&cert_path, &key_path).await?;

    // Spawn a task for each listener
    let mut handles = Vec::new();

    for listener_config in config.listeners {
        let rustls_config = rustls_config.clone();
        let http_client = http_client.clone();
        let client_tls_config = ws_client_config.clone();
        let target = listener_config.target.clone();
        let port = listener_config.port;

        let handle = tokio::spawn(async move {
            let addr = SocketAddr::from(([0, 0, 0, 0], port));

            // Create router with the target baked in
            let app = Router::new().fallback(any({
                let target = target.clone();
                let http_client = http_client.clone();
                let client_tls_config = client_tls_config.clone();
                move |connect_info, req| {
                    let target = target.clone();
                    let http_client = http_client.clone();
                    let client_tls_config = client_tls_config.clone();
                    async move {
                        proxy_handler(connect_info, req, target, http_client, client_tls_config)
                            .await
                    }
                }
            }));

            tracing::info!("HTTPS listener on :{} -> {}", port, target);

            if let Err(e) = axum_server::bind_rustls(addr, rustls_config)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await
            {
                tracing::error!("Listener on port {} failed: {}", port, e);
            }
        });

        handles.push(handle);
    }

    // Wait for all listeners (they should run forever)
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}
