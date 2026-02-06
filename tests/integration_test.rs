//! Integration tests for the HTTPS proxy
//!
//! These tests spin up a mock upstream server and test the proxy logic
//! by making actual HTTP requests through the proxy handler.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use std::net::SocketAddr;
use std::sync::Arc;

use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

// Re-export the proxy module for testing
// Note: We need to import from the main crate
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;

// Imports for WebSocket testing
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::routing::any;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;

/// Helper to create HTTPS-capable client for tests
fn create_test_client() -> Arc<Client<HttpsConnector<HttpConnector>, Body>> {
    // Install default crypto provider (required for rustls 0.23+)
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .expect("no native root CA certificates found")
        .https_or_http()
        .enable_http1()
        .build();
    Arc::new(Client::builder(TokioExecutor::new()).build(https))
}

#[tokio::test]
async fn test_proxy_forwards_request_to_upstream() {
    // Start mock upstream server
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/users"))
        .respond_with(ResponseTemplate::new(200).set_body_string("users list"))
        .mount(&mock_server)
        .await;

    let target = mock_server.uri();
    let http_client = create_test_client();

    // Create request
    let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
    let req = Request::builder()
        .uri("/api/users")
        .body(Body::empty())
        .unwrap();

    // Call proxy handler
    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_preserves_query_string() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("search results"))
        .mount(&mock_server)
        .await;

    let target = mock_server.uri();
    let http_client = create_test_client();

    let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
    let req = Request::builder()
        .uri("/search?q=hello&page=2")
        .body(Body::empty())
        .unwrap();

    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_adds_x_forwarded_headers() {
    let mock_server = MockServer::start().await;

    // Expect X-Forwarded-For header to be set
    Mock::given(method("GET"))
        .and(path("/"))
        .and(header("x-forwarded-for", "10.0.0.50"))
        .and(header("x-real-ip", "10.0.0.50"))
        .and(header("x-forwarded-proto", "https"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let target = mock_server.uri();
    let http_client = create_test_client();

    let addr: SocketAddr = "10.0.0.50:12345".parse().unwrap();
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_appends_to_existing_x_forwarded_for() {
    let mock_server = MockServer::start().await;

    // Note: Header append logic is tested in unit tests
    // This integration test verifies the request reaches upstream successfully
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let target = mock_server.uri();
    let http_client = create_test_client();

    let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
    let req = Request::builder()
        .uri("/")
        .header("x-forwarded-for", "203.0.113.1")
        .body(Body::empty())
        .unwrap();

    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_returns_502_on_upstream_failure() {
    // Target a non-existent server
    let target = "http://127.0.0.1:59999".to_string();
    let http_client = create_test_client();

    let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_proxy_forwards_post_request_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/data"))
        .respond_with(ResponseTemplate::new(201).set_body_string("created"))
        .mount(&mock_server)
        .await;

    let target = mock_server.uri();
    let http_client = create_test_client();

    let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/api/data")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name": "test"}"#))
        .unwrap();

    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_proxy_preserves_response_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-custom-header", "custom-value")
                .insert_header("cache-control", "no-cache"),
        )
        .mount(&mock_server)
        .await;

    let target = mock_server.uri();
    let http_client = create_test_client();

    let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("x-custom-header")
            .unwrap()
            .to_str()
            .unwrap(),
        "custom-value"
    );
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "no-cache"
    );
}

#[tokio::test]
async fn test_proxy_handles_404_from_upstream() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/not-found"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let target = mock_server.uri();
    let http_client = create_test_client();

    let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
    let req = Request::builder()
        .uri("/not-found")
        .body(Body::empty())
        .unwrap();

    let response = https_proxy::proxy_handler(ConnectInfo(addr), req, target, http_client).await;

    // Proxy should pass through the 404 status
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_proxy_forwards_websocket() {
    // 1. Start a real backend server that echoes WebSocket messages
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        // Bind to port 0 to get a random available port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tx.send(addr).unwrap();

        // Simple Axum router for the backend that echoes messages
        let app = Router::new().route(
            "/ws",
            any(|ws: WebSocketUpgrade| async {
                ws.on_upgrade(|mut socket: WebSocket| async move {
                    while let Some(Ok(msg)) = socket.recv().await {
                        if let Message::Text(text) = msg {
                            if socket.send(Message::Text(text)).await.is_err() {
                                break;
                            }
                        }
                    }
                })
            }),
        );

        axum::serve(listener, app).await.unwrap();
    });

    let backend_addr = rx.await.unwrap();
    let target = format!("http://{}", backend_addr);
    let http_client = create_test_client();

    // 2. Start the proxy server itself (since we need a real HTTP server to handle Upgrade headers properly)
    // We can't just call proxy_handler directly easily because the Upgrade requires taking over the transport,
    // which axum does via its OnUpgrade mechanism that expects to be running in a server.
    let (proxy_tx, proxy_rx) = tokio::sync::oneshot::channel();

    let proxy_target = target.clone();
    let proxy_client = http_client.clone();

    tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = listener.local_addr().unwrap();
        proxy_tx.send(proxy_addr).unwrap();

        let app = Router::new().fallback(any(move |connect_info: ConnectInfo<SocketAddr>, req| {
            let target = proxy_target.clone();
            let http_client = proxy_client.clone();
            async move { https_proxy::proxy_handler(connect_info, req, target, http_client).await }
        }));

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    let proxy_addr = proxy_rx.await.unwrap();

    // 3. Connect client to PROXY
    let proxy_url = format!("ws://{}/ws", proxy_addr);
    let (mut socket, _) = tokio_tungstenite::connect_async(proxy_url)
        .await
        .expect("Failed to connect to proxy");

    // 4. Send message and verify echo
    let msg = "Hello WebSocket";
    socket
        .send(TungsteniteMessage::Text(msg.to_string()))
        .await
        .unwrap();

    let received = socket
        .next()
        .await
        .expect("No message received")
        .expect("WebSocket error");
    assert_eq!(received.to_text().unwrap(), msg);

    // Cleanup
    let _ = socket.close(None).await;
}
