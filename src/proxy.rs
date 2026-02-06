use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, HeaderValue, Request, Response, StatusCode, Uri, Version},
};
use hyper_util::client::legacy::Client;
use std::net::SocketAddr;
use std::sync::Arc;

type HttpClient = Arc<Client<hyper_util::client::legacy::connect::HttpConnector, Body>>;

/// Main proxy handler - forwards requests to the configured target
pub async fn proxy_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    target: String,
    http_client: HttpClient,
) -> Response<Body> {
    let method = req.method().clone();

    tracing::info!("Proxying {} {} -> {}", method, req.uri(), target);

    // Check for WebSocket upgrade
    if is_websocket_upgrade(&req) {
        return handle_websocket_upgrade(req, &target, addr, &http_client).await;
    }

    // Forward regular HTTP request
    forward_request(req, &target, addr, &http_client).await
}

/// Helper function to check if a header contains a specific value (case-insensitive)
fn header_contains<B>(req: &Request<B>, name: &str, value: &str) -> bool {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains(value))
        .unwrap_or(false)
}

/// Check if request is a WebSocket upgrade
fn is_websocket_upgrade<B>(req: &Request<B>) -> bool {
    header_contains(req, "connection", "upgrade") && header_contains(req, "upgrade", "websocket")
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
            format!("{}, {}", existing_str, client_addr.ip())
        }
        None => client_addr.ip().to_string(),
    };
    headers.insert("x-forwarded-for", HeaderValue::from_str(&xff)?);

    // X-Forwarded-Proto
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));

    // X-Forwarded-Host (from original Host header)
    if let Some(host) = original_host {
        headers.insert("x-forwarded-host", host);
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
    let original_host = parts.headers.get("host").cloned();

    // Update URI
    parts.uri = upstream_uri;

    // Force HTTP/1.1 for upstream requests
    parts.version = Version::HTTP_11;

    // Add forwarding headers
    add_forwarding_headers(&mut parts.headers, client_addr, original_host)?;

    // Remove hop-by-hop headers
    remove_hop_by_hop_headers(&mut parts.headers);

    Ok(Request::from_parts(parts, body))
}

/// Handle WebSocket upgrade request
async fn handle_websocket_upgrade(
    req: Request<Body>,
    target: &str,
    client_addr: SocketAddr,
    http_client: &HttpClient,
) -> Response<Body> {
    tracing::info!("WebSocket upgrade request from {}", client_addr);

    // Build upstream URI
    let upstream_uri = match build_upstream_uri(req.uri(), target) {
        Ok(uri) => uri,
        Err(e) => {
            tracing::error!("Failed to build upstream URI for WebSocket: {}", e);
            return bad_gateway_response(&format!("Invalid WebSocket upstream URI: {}", e));
        }
    };

    // For WebSocket proxying, we need to keep connection and upgrade headers
    let (mut parts, body) = req.into_parts();

    // Save original host
    let original_host = parts.headers.get("host").cloned();

    parts.uri = upstream_uri;

    // Add X-Forwarded headers (but don't remove hop-by-hop for WebSocket)
    if let Err(e) = add_forwarding_headers(&mut parts.headers, client_addr, original_host) {
        tracing::error!("Failed to add forwarding headers: {}", e);
        return bad_gateway_response(&format!("Header error: {}", e));
    }

    let upstream_req = Request::from_parts(parts, body);

    // Send upgrade request to upstream
    match http_client.request(upstream_req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let body = Body::new(body);
            Response::from_parts(parts, body)
        }
        Err(e) => {
            tracing::error!("WebSocket upstream request failed: {}", e);
            bad_gateway_response(&format!("WebSocket upstream failed: {}", e))
        }
    }
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

    // ==================== is_websocket_upgrade tests ====================

    #[test]
    fn test_is_websocket_upgrade_true() {
        let req = Request::builder()
            .header("connection", "Upgrade")
            .header("upgrade", "websocket")
            .body(())
            .unwrap();

        assert!(is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_websocket_upgrade_case_insensitive() {
        let req = Request::builder()
            .header("Connection", "UPGRADE")
            .header("Upgrade", "WebSocket")
            .body(())
            .unwrap();

        assert!(is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_websocket_upgrade_false_no_headers() {
        let req = Request::builder().body(()).unwrap();

        assert!(!is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_websocket_upgrade_false_missing_upgrade() {
        let req = Request::builder()
            .header("connection", "upgrade")
            .body(())
            .unwrap();

        assert!(!is_websocket_upgrade(&req));
    }

    #[test]
    fn test_is_websocket_upgrade_false_wrong_upgrade_type() {
        let req = Request::builder()
            .header("connection", "upgrade")
            .header("upgrade", "h2c")
            .body(())
            .unwrap();

        assert!(!is_websocket_upgrade(&req));
    }

    // ==================== header_contains tests ====================

    #[test]
    fn test_header_contains_found() {
        let req = Request::builder()
            .header("content-type", "application/json")
            .body(())
            .unwrap();

        assert!(header_contains(&req, "content-type", "json"));
    }

    #[test]
    fn test_header_contains_not_found() {
        let req = Request::builder()
            .header("content-type", "text/html")
            .body(())
            .unwrap();

        assert!(!header_contains(&req, "content-type", "json"));
    }

    #[test]
    fn test_header_contains_missing_header() {
        let req = Request::builder().body(()).unwrap();

        assert!(!header_contains(&req, "content-type", "json"));
    }

    // ==================== build_upstream_uri tests ====================

    #[test]
    fn test_build_upstream_uri_simple_path() {
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
    fn test_build_upstream_uri_root_path() {
        let original: Uri = "/".parse().unwrap();
        let target = "http://backend:8080";

        let result = build_upstream_uri(&original, target).unwrap();

        assert_eq!(result.to_string(), "http://backend:8080/");
    }

    #[test]
    fn test_build_upstream_uri_empty_path_defaults_to_root() {
        // When original URI has no path, it should default to "/"
        let original: Uri = "http://localhost".parse().unwrap();
        let target = "http://backend:8080";

        let result = build_upstream_uri(&original, target).unwrap();

        // path_and_query returns None for URI without path, which defaults to "/"
        assert_eq!(result.to_string(), "http://backend:8080/");
    }

    #[test]
    fn test_build_upstream_uri_preserves_https() {
        let original: Uri = "/api".parse().unwrap();
        let target = "https://secure-backend:443";

        let result = build_upstream_uri(&original, target).unwrap();

        assert_eq!(result.to_string(), "https://secure-backend:443/api");
    }

    // ==================== add_forwarding_headers tests ====================

    #[test]
    fn test_add_forwarding_headers_new() {
        let mut headers = HeaderMap::new();
        let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();

        add_forwarding_headers(&mut headers, addr, None).unwrap();

        assert_eq!(
            headers.get("x-real-ip").unwrap().to_str().unwrap(),
            "192.168.1.100"
        );
        assert_eq!(
            headers.get("x-forwarded-for").unwrap().to_str().unwrap(),
            "192.168.1.100"
        );
        assert_eq!(
            headers.get("x-forwarded-proto").unwrap().to_str().unwrap(),
            "https"
        );
    }

    #[test]
    fn test_add_forwarding_headers_appends_xff() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("10.0.0.1"));
        let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();

        add_forwarding_headers(&mut headers, addr, None).unwrap();

        assert_eq!(
            headers.get("x-forwarded-for").unwrap().to_str().unwrap(),
            "10.0.0.1, 192.168.1.100"
        );
    }

    #[test]
    fn test_add_forwarding_headers_with_host() {
        let mut headers = HeaderMap::new();
        let addr: SocketAddr = "192.168.1.100:54321".parse().unwrap();
        let host = HeaderValue::from_static("example.com");

        add_forwarding_headers(&mut headers, addr, Some(host)).unwrap();

        assert_eq!(
            headers.get("x-forwarded-host").unwrap().to_str().unwrap(),
            "example.com"
        );
    }

    // ==================== remove_hop_by_hop_headers tests ====================

    #[test]
    fn test_remove_hop_by_hop_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("connection", HeaderValue::from_static("keep-alive"));
        headers.insert("keep-alive", HeaderValue::from_static("timeout=5"));
        headers.insert("transfer-encoding", HeaderValue::from_static("chunked"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        remove_hop_by_hop_headers(&mut headers);

        assert!(headers.get("connection").is_none());
        assert!(headers.get("keep-alive").is_none());
        assert!(headers.get("transfer-encoding").is_none());
        // Non-hop-by-hop headers should remain
        assert!(headers.get("content-type").is_some());
    }

    // ==================== bad_gateway_response tests ====================

    #[test]
    fn test_bad_gateway_response_status() {
        let resp = bad_gateway_response("Test error");

        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_bad_gateway_response_content_type() {
        let resp = bad_gateway_response("Test error");

        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/plain; charset=utf-8"
        );
    }
}
