use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, HeaderValue, Request, Response, StatusCode, Uri, Version},
};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use rustls::ClientConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_tungstenite::Connector;

type HttpClient = Arc<Client<HttpsConnector<HttpConnector>, Body>>;

/// Main proxy handler - forwards requests to the configured target
pub async fn proxy_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    target: String,
    http_client: HttpClient,
    tls_config: Arc<ClientConfig>,
) -> Response<Body> {
    let method = req.method().clone();

    tracing::info!("Proxying {} {} -> {}", method, req.uri(), target);

    // Check for WebSocket upgrade
    if is_websocket_upgrade(&req) {
        return handle_websocket_upgrade(req, &target, addr, &http_client, &tls_config).await;
    }

    // Forward regular HTTP request
    forward_request(req, &target, addr, &http_client).await
}

/// Helper function to check if a header contains a specific value (case-insensitive)
fn header_contains<B>(req: &Request<B>, name: &str, value: &str) -> bool {
    req.headers()
        .get_all(name)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| v.to_lowercase().contains(value))
}

/// Check if request is a WebSocket upgrade
fn is_websocket_upgrade<B>(req: &Request<B>) -> bool {
    header_contains(req, "connection", "upgrade") && header_contains(req, "upgrade", "websocket")
}

/// Normalize Cookie headers:
/// - Browsers over HTTP/2 can send multiple `cookie` headers.
/// - Some upstream stacks (PHP/FPM, certain servers) may parse only the last one or parse wrongly.
/// - Fix: merge into a single Cookie header joined by "; ".
fn normalize_cookie_headers(headers: &mut HeaderMap) -> anyhow::Result<()> {
    let cookies: Vec<&str> = headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if cookies.len() <= 1 {
        return Ok(());
    }

    // Cookie must be merged with "; " (NOT ",")
    let merged = cookies.join("; ");

    headers.remove("cookie");
    headers.insert("cookie", HeaderValue::from_str(&merged)?);

    Ok(())
}

/// Add X-Forwarded-* and X-Real-IP headers to the request
fn add_forwarding_headers(
    headers: &mut HeaderMap,
    client_addr: SocketAddr,
    original_host: Option<HeaderValue>,
) -> anyhow::Result<()> {
    // X-Real-IP - the actual client IP
    headers.insert(
        "x-real-ip",
        HeaderValue::from_str(&client_addr.ip().to_string())?,
    );

    // X-Forwarded-For - append to existing or create new
    let xff = match headers.get("x-forwarded-for") {
        Some(existing) => {
            let existing_str = existing.to_str().unwrap_or("");
            if existing_str.trim().is_empty() {
                client_addr.ip().to_string()
            } else {
                format!("{}, {}", existing_str, client_addr.ip())
            }
        }
        None => client_addr.ip().to_string(),
    };
    headers.insert("x-forwarded-for", HeaderValue::from_str(&xff)?);

    // X-Forwarded-Proto
    // Only set if not already present (e.g. when behind another LB/proxy).
    if !headers.contains_key("x-forwarded-proto") {
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
    }

    // X-Forwarded-Host (from original Host header)
    if let Some(host) = original_host {
        if !headers.contains_key("x-forwarded-host") {
            headers.insert("x-forwarded-host", host);
        }
    }

    // X-Forwarded-Port
    if !headers.contains_key("x-forwarded-port") {
        headers.insert("x-forwarded-port", HeaderValue::from_static("443"));
    }

    Ok(())
}

/// Remove hop-by-hop headers that shouldn't be forwarded
fn remove_hop_by_hop_headers(headers: &mut HeaderMap) {
    headers.remove("connection");
    headers.remove("keep-alive");
    headers.remove("proxy-authenticate");
    headers.remove("proxy-authorization");
    headers.remove("te");
    headers.remove("trailers");
    headers.remove("transfer-encoding");
    headers.remove("upgrade");
}

/// Forward HTTP request to upstream
async fn forward_request(
    req: Request<Body>,
    target: &str,
    client_addr: SocketAddr,
    http_client: &HttpClient,
) -> Response<Body> {
    // Build upstream URI - preserve full path and query string
    let upstream_uri = match build_upstream_uri(req.uri(), target) {
        Ok(uri) => uri,
        Err(e) => {
            tracing::error!("Failed to build upstream URI: {}", e);
            return bad_gateway_response(&format!("Invalid upstream URI: {}", e));
        }
    };

    // Build new request with forwarding headers
    let upstream_req = match build_upstream_request(req, upstream_uri, client_addr) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to build upstream request: {}", e);
            return bad_gateway_response(&format!("Failed to build request: {}", e));
        }
    };

    // Log upstream headers if debug is enabled
    if tracing::enabled!(tracing::Level::DEBUG) {
        tracing::debug!("Upstream request headers:");
        for (name, value) in upstream_req.headers().iter() {
            tracing::debug!("  {}: {:?}", name, value);
        }

        let cookie_count = upstream_req.headers().get_all("cookie").iter().count();
        tracing::debug!("cookie header count = {}", cookie_count);
    }

    // Send request to upstream
    match http_client.request(upstream_req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let body = Body::new(body);
            Response::from_parts(parts, body)
        }
        Err(e) => {
            tracing::error!("Upstream request failed: {}", e);
            bad_gateway_response(&format!("Upstream connection failed: {}", e))
        }
    }
}

/// Build the upstream URI - preserving full path and query
fn build_upstream_uri(original: &Uri, target: &str) -> anyhow::Result<Uri> {
    // Parse target URL
    let target_uri: Uri = target.parse()?;

    // Build path + query (NOT stripping/rewriting anything)
    let path_and_query = original
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    // Construct full URI
    let uri_str = format!(
        "{}://{}{}",
        target_uri.scheme_str().unwrap_or("http"),
        target_uri.authority().map(|a| a.as_str()).unwrap_or(""),
        path_and_query,
    );

    Ok(uri_str.parse()?)
}

/// Build the upstream request with X-Forwarded-* headers
fn build_upstream_request(
    req: Request<Body>,
    upstream_uri: Uri,
    client_addr: SocketAddr,
) -> anyhow::Result<Request<Body>> {
    let (mut parts, body) = req.into_parts();

    // Save original host before modifying headers
    let original_host = parts.headers.get("host").cloned().or_else(|| {
        parts
            .uri
            .authority()
            .and_then(|auth| HeaderValue::from_str(auth.as_str()).ok())
    });

    // Update URI
    parts.uri = upstream_uri;

    // Force HTTP/1.1 for upstream requests (optional, but ok)
    parts.version = Version::HTTP_11;

    // Add forwarding headers
    add_forwarding_headers(&mut parts.headers, client_addr, original_host)?;

    // Remove hop-by-hop headers
    remove_hop_by_hop_headers(&mut parts.headers);

    // ✅ IMPORTANT: normalize Cookie headers for upstream compatibility
    normalize_cookie_headers(&mut parts.headers)?;

    Ok(Request::from_parts(parts, body))
}

/// Handle WebSocket upgrade request
async fn handle_websocket_upgrade(
    mut req: Request<Body>,
    target: &str,
    client_addr: SocketAddr,
    _http_client: &HttpClient,
    tls_config: &Arc<ClientConfig>,
) -> Response<Body> {
    tracing::info!("WebSocket upgrade request from {}", client_addr);

    // 1. Build upstream WebSocket URL (ws:// or wss://)
    let target_uri: Uri = match target.parse() {
        Ok(uri) => uri,
        Err(e) => return bad_gateway_response(&format!("Invalid target URI: {}", e)),
    };

    let scheme = match target_uri.scheme_str() {
        Some("https") => "wss",
        _ => "ws",
    };

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let authority = target_uri.authority().map(|a| a.as_str()).unwrap_or("");

    let upstream_url = format!("{}://{}{}", scheme, authority, path_and_query);

    // 2. Prepare upgrade response for the client
    let upgrade_header = match req.headers().get("sec-websocket-key") {
        Some(h) => h,
        None => return bad_gateway_response("Missing Sec-WebSocket-Key"),
    };

    // Calculate accept key
    let accept_key =
        tokio_tungstenite::tungstenite::handshake::derive_accept_key(upgrade_header.as_bytes());

    let response = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Accept", accept_key)
        .body(Body::empty())
        .unwrap();

    // 3. Spawn task to handle the tunnel
    let tls_config = tls_config.clone();
    tokio::spawn(async move {
        // Wait for the client connection to be upgraded
        match hyper::upgrade::on(&mut req).await {
            Ok(upgraded) => {
                // Convert upgraded connection to TokioIo for tungstenite
                let upgraded = hyper_util::rt::TokioIo::new(upgraded);

                // Connect to upstream using the insecure TLS config
                let connector = Connector::Rustls(tls_config.clone());
                match tokio_tungstenite::connect_async_tls_with_config(
                    upstream_url,
                    None,
                    false,
                    Some(connector),
                )
                .await
                {
                    Ok((ws_stream, _)) => {
                        // Create client WebSocket stream from the upgraded connection
                        // from_raw_socket is async in tokio-tungstenite and returns WebSocketStream
                        let client_ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
                            upgraded,
                            tokio_tungstenite::tungstenite::protocol::Role::Server,
                            None,
                        )
                        .await;

                        use futures_util::{SinkExt, StreamExt};

                        let (mut client_write, mut client_read) = client_ws_stream.split();
                        let (mut upstream_write, mut upstream_read) = ws_stream.split();

                        // Forward messages: client -> upstream
                        let client_to_upstream = async {
                            while let Some(msg) = client_read.next().await {
                                match msg {
                                    Ok(msg) => {
                                        if let Err(e) = upstream_write.send(msg).await {
                                            tracing::error!("Failed to send to upstream: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Client WS error: {}", e);
                                        break;
                                    }
                                }
                            }
                        };

                        // Forward messages: upstream -> client
                        let upstream_to_client = async {
                            while let Some(msg) = upstream_read.next().await {
                                match msg {
                                    Ok(msg) => {
                                        if let Err(e) = client_write.send(msg).await {
                                            tracing::error!("Failed to send to client: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Upstream WS error: {}", e);
                                        break;
                                    }
                                }
                            }
                        };

                        tokio::select! {
                            _ = client_to_upstream => {},
                            _ = upstream_to_client => {},
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to connect to upstream WebSocket: {}", e);
                    }
                }
            }
            Err(e) => tracing::error!("Upgrade error: {}", e),
        }
    });

    response
}

/// 502 Bad Gateway response with detailed message
fn bad_gateway_response(message: &str) -> Response<Body> {
    tracing::warn!("Returning 502: {}", message);
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Body::from(format!("502 Bad Gateway - {}", message)))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[test]
    fn test_header_contains_single_value() {
        let req = Request::builder()
            .header("connection", "upgrade")
            .body(())
            .unwrap();
        assert!(header_contains(&req, "connection", "upgrade"));
    }

    #[test]
    fn test_header_contains_case_insensitive() {
        let req = Request::builder()
            .header("Connection", "Upgrade")
            .body(())
            .unwrap();
        assert!(header_contains(&req, "connection", "upgrade"));
    }

    #[test]
    fn test_header_contains_multiple_values() {
        let req = Request::builder()
            .header("connection", "keep-alive, upgrade")
            .body(())
            .unwrap();
        assert!(header_contains(&req, "connection", "upgrade"));
        assert!(header_contains(&req, "connection", "keep-alive"));
    }

    #[test]
    fn test_header_contains_not_found() {
        let req = Request::builder()
            .header("connection", "keep-alive")
            .body(())
            .unwrap();
        assert!(!header_contains(&req, "connection", "upgrade"));
    }

    #[test]
    fn test_is_websocket_upgrade_valid() {
        let req = Request::builder()
            .header("connection", "upgrade")
            .header("upgrade", "websocket")
            .body(())
            .unwrap();
        assert!(is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_websocket_upgrade_missing_connection() {
        let req = Request::builder()
            .header("upgrade", "websocket")
            .body(())
            .unwrap();
        assert!(!is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_websocket_upgrade_missing_upgrade() {
        let req = Request::builder()
            .header("connection", "upgrade")
            .body(())
            .unwrap();
        assert!(!is_websocket_upgrade(&req));
    }

    #[test]
    fn test_normalize_cookie_headers_single() {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", HeaderValue::from_static("session=abc"));
        normalize_cookie_headers(&mut headers).unwrap();
        assert_eq!(headers.get("cookie").unwrap(), "session=abc");
    }

    #[test]
    fn test_normalize_cookie_headers_multiple() {
        let mut headers = HeaderMap::new();
        headers.append("cookie", HeaderValue::from_static("session=abc"));
        headers.append("cookie", HeaderValue::from_static("token=xyz"));
        normalize_cookie_headers(&mut headers).unwrap();
        assert_eq!(headers.get("cookie").unwrap(), "session=abc; token=xyz");
        assert_eq!(headers.get_all("cookie").iter().count(), 1);
    }

    #[test]
    fn test_normalize_cookie_headers_empty() {
        let mut headers = HeaderMap::new();
        normalize_cookie_headers(&mut headers).unwrap();
        assert!(headers.get("cookie").is_none());
    }

    #[test]
    fn test_add_forwarding_headers_new() {
        let mut headers = HeaderMap::new();
        let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
        let original_host = Some(HeaderValue::from_static("example.com"));
        add_forwarding_headers(&mut headers, addr, original_host).unwrap();

        assert_eq!(headers.get("x-real-ip").unwrap(), "192.168.1.100");
        assert_eq!(headers.get("x-forwarded-for").unwrap(), "192.168.1.100");
        assert_eq!(headers.get("x-forwarded-proto").unwrap(), "https");
        assert_eq!(headers.get("x-forwarded-host").unwrap(), "example.com");
        assert_eq!(headers.get("x-forwarded-port").unwrap(), "443");
    }

    #[test]
    fn test_add_forwarding_headers_append_xff() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("10.0.0.1, 10.0.0.2"),
        );
        let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
        add_forwarding_headers(&mut headers, addr, None).unwrap();

        assert_eq!(
            headers.get("x-forwarded-for").unwrap(),
            "10.0.0.1, 10.0.0.2, 192.168.1.100"
        );
    }

    #[test]
    fn test_add_forwarding_headers_preserves_existing_proto() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-proto", HeaderValue::from_static("http"));
        let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
        add_forwarding_headers(&mut headers, addr, None).unwrap();

        assert_eq!(headers.get("x-forwarded-proto").unwrap(), "http");
    }

    #[test]
    fn test_remove_hop_by_hop_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("connection", HeaderValue::from_static("keep-alive"));
        headers.insert("keep-alive", HeaderValue::from_static("timeout=5"));
        headers.insert("transfer-encoding", HeaderValue::from_static("chunked"));
        headers.insert("upgrade", HeaderValue::from_static("websocket"));
        headers.insert("content-type", HeaderValue::from_static("text/plain"));
        headers.insert("x-custom", HeaderValue::from_static("value"));

        remove_hop_by_hop_headers(&mut headers);

        assert!(headers.get("connection").is_none());
        assert!(headers.get("keep-alive").is_none());
        assert!(headers.get("transfer-encoding").is_none());
        assert!(headers.get("upgrade").is_none());
        assert!(headers.get("content-type").is_some());
        assert!(headers.get("x-custom").is_some());
    }

    #[test]
    fn test_build_upstream_uri_simple() {
        let original: Uri = "/api/users".parse().unwrap();
        let target = "http://backend:8080";
        let result = build_upstream_uri(&original, target).unwrap();
        assert_eq!(result.to_string(), "http://backend:8080/api/users");
    }

    #[test]
    fn test_build_upstream_uri_with_query() {
        let original: Uri = "/search?q=hello&page=2".parse().unwrap();
        let target = "http://backend:8080";
        let result = build_upstream_uri(&original, target).unwrap();
        assert_eq!(
            result.to_string(),
            "http://backend:8080/search?q=hello&page=2"
        );
    }

    #[test]
    fn test_build_upstream_uri_https() {
        let original: Uri = "/api".parse().unwrap();
        let target = "https://api.example.com";
        let result = build_upstream_uri(&original, target).unwrap();
        assert_eq!(result.to_string(), "https://api.example.com/api");
    }

    #[test]
    fn test_build_upstream_uri_root_path() {
        let original: Uri = "/".parse().unwrap();
        let target = "http://backend:3000";
        let result = build_upstream_uri(&original, target).unwrap();
        assert_eq!(result.to_string(), "http://backend:3000/");
    }

    #[test]
    fn test_bad_gateway_response() {
        let response = bad_gateway_response("Connection refused");
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain; charset=utf-8"
        );
    }
}
