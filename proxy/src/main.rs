use https_proxy::config::Config;
use https_proxy::proxy_handler;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{routing::any, Router};
use axum_server::tls_rustls::RustlsConfig;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const CERT_PATH: &str = "/certs/cert.pem";
const KEY_PATH: &str = "/certs/key.pem";
const CONFIG_PATH: &str = "/etc/proxy/routes.yaml";

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
    tracing::info!("Loading config from: {}", CONFIG_PATH);
    let config = Config::load(CONFIG_PATH)?;
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
    tracing::info!("Loading TLS cert from: {}", CERT_PATH);
    tracing::info!("Loading TLS key from: {}", KEY_PATH);

    let rustls_config = RustlsConfig::from_pem_file(CERT_PATH, KEY_PATH).await?;

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
                        proxy_handler(
                            connect_info,
                            req,
                            target,
                            http_client,
                            client_tls_config,
                            port,
                        )
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

    // Wait for shutdown signal or listener failure
    tokio::select! {
        _ = async {
            for handle in handles {
                let _ = handle.await;
            }
        } => {
            tracing::warn!("All listeners stopped unexpectedly");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received shutdown signal, stopping...");
        }
    }

    Ok(())
}
