# Project Architecture

## Overview

HTTPS Proxy is a lightweight, high-performance reverse proxy written in Rust, designed for local development. It provides HTTPS termination for services running in Docker Compose.

## Architecture Diagram

```
                    ┌─────────────────────────────────────────┐
                    │         Client (Browser/curl)           │
                    │              wss://localhost            │
                    └──────────────────┬──────────────────────┘
                                       │
                                       │ TLS (HTTPS)
                                       ▼
┌────────────────────────────────────────────────────────────────┐
│                     Docker Container                           │
│  ┌────────────────────────────────────────────────────────┐    │
│  │              https-proxy (Rust Application)            │    │
│  │                                                        │    │
│  │   ┌──────────────┐    ┌──────────────┐                 │    │
│  │   │ Listener :440│    │ Listener :443│                 │    │
│  │   │  (HTTPS)     │    │  (WebSocket) │                 │    │
│  │   └──────┬───────┘    └──────┬───────┘                 │    │
│  │          │                    │                        │    │
│  │          ▼                    ▼                        │    │
│  │   ┌──────────────┐    ┌──────────────┐                 │    │
│  │   │ HTTP Client  │    │    WS        │                 │    │
│  │   │ (hyper)      │    │  Tunnel      │                 │    │
│  │   └──────┬───────┘    └──────┬───────┘                 │    │
│  │          │                    │                        │    │
│  └──────────┼────────────────────┼────────────────────────┘    │
│             │                    │                             │
│             │  http://          │  ws://                       │
│             ▼                    ▼                             │
│  ┌─────────────────┐    ┌─────────────────┐                   │
│  │   api:3000      │    │  ws-echo:8080   │                   │
│  │   (whoami)      │    │  (echo-server)  │                   │
│  └─────────────────┘    └─────────────────┘                   │
└────────────────────────────────────────────────────────────────┘
```

## Main Components

### 1. proxy crate

The core component of the application, handling proxy requests.

#### main.rs

Application entry point:

- Initialize tracing/logging
- Load configuration from `routes.yaml`
- Create TLS config for server and client
- Spawn multiple listeners (one task per port)
- Handle shutdown signals

```rust
// Pseudo-code: Main flow
fn main() {
    // 1. Load config
    let config = Config::load("/etc/proxy/routes.yaml")?;

    // 2. Create HTTP client with insecure TLS (for upstream HTTPS)
    let http_client = create_https_client();

    // 3. Load server TLS certificates
    let rustls_config = RustlsConfig::from_pem_file(...)?;

    // 4. Spawn listeners for each port
    for listener in config.listeners {
        spawn_https_listener(listener.port, listener.target);
    }

    // 5. Wait for shutdown
    wait_for_ctrl_c();
}
```

#### config.rs

Defines configuration structure:

```rust
pub struct Listener {
    pub port: u16,      // Listening port
    pub target: String, // Upstream URL (http://, https://, ws://, wss://)
}

pub struct Config {
    pub listeners: Vec<Listener>,
}
```

#### proxy.rs

Core proxy logic:

1. **HTTP Proxying**: Forward requests from client to upstream
2. **WebSocket Tunneling**: Upgrade and tunnel WebSocket connections
3. **Header manipulation**:
   - Add `X-Forwarded-*` headers
   - Normalize Cookie headers
   - Remove hop-by-hop headers
4. **TLS handling**: Use insecure client config for upstream HTTPS

#### tls.rs

TLS configuration for upstream connections:

```rust
// Verifier that accepts all certificates
// WARNING: Only for local development!
pub struct NoCertificateVerification;

pub fn get_insecure_client_config() -> ClientConfig {
    // Create config that doesn't verify server certificates
}
```

### 2. manage-ca crate

CLI tool for managing CA certificates on the system.

#### Features

- **Install**: Add CA to system trust store and browsers
- **Uninstall**: Remove CA from the system

#### Supported Platforms

| OS      | System Store                                 | Browsers                    |
| ------- | -------------------------------------------- | --------------------------- |
| Linux   | `update-ca-certificates` / `update-ca-trust` | Firefox (NSS), Chrome (NSS) |
| macOS   | Keychain                                     | Firefox                     |
| Windows | Certificate Store                            | Firefox, Chrome             |

### 3. entrypoint.sh

Shell script that runs before the application starts:

1. **Create CA certificate** (if not exists)
   - Auto-generate key and self-signed cert
   - Valid: 10 years

2. **Install CA into container**
   - Copy to `/usr/local/share/ca-certificates`
   - Run `update-ca-certificates`

3. **Create server certificate** (if not exists)
   - Generate key and CSR
   - Sign by CA
   - Valid: 1 year
   - SANs: localhost, *.localhost, *.local, 127.0.0.1, ::1

### 4. Dockerfile

Multi-stage build:

```dockerfile
# Build stage
FROM rust:1.93.0-alpine3.23 AS builder
# Build proxy binary with LTO

# Runtime stage
FROM alpine:3.23
# Only install runtime dependencies
# ~7MB final image
```

## Request Processing Flow

### HTTP Request

```
1. Client sends HTTPS request to proxy
              │
              ▼
2. Proxy terminates TLS (server cert)
              │
              ▼
3. Axum router receives request
              │
              ▼
4. proxy_handler() is called
              │
              ▼
5. Check WebSocket upgrade?
   ├── Yes → handle_websocket_upgrade()
   └── No  → forward_request()
              │
              ▼
6. Build upstream request:
   - Build URI (preserve path/query)
   - Add X-Forwarded-* headers
   - Normalize cookies
   - Remove hop-by-hop headers
              │
              ▼
7. Send request via HTTP client
   (supports both http:// and https:// upstream)
              │
              ▼
8. Response is forwarded to client
```

### WebSocket Request

```
1. Client sends WebSocket upgrade request over HTTPS
              │
              ▼
2. Proxy receives request, detects WebSocket:
   - Connection: upgrade
   - Upgrade: websocket
              │
              ▼
3. Response 101 Switching Protocols
              │
              ▼
4. Tunnel is created:
   - Client ↔ Proxy ↔ Upstream (ws:// or wss://)
              │
              ▼
5. Bidirectional message forwarding
```

## Main Dependencies

| Crate               | Purpose                     |
| ------------------- | -------------------------- |
| `axum`              | Web framework, HTTP server |
| `hyper`             | HTTP/1.1 client & server   |
| `rustls`            | TLS/SSL                    |
| `tokio-tungstenite` | WebSocket                  |
| `serde_yaml`        | Parse YAML config          |
| `tokio`             | Async runtime              |

## Security Considerations

⚠️ **Warning**: This is a tool for local development:

1. **Insecure TLS verification**: Proxy accepts all upstream certificates
2. **Self-signed CA**: Do not install to production systems
3. **No authentication**: No auth mechanism
4. **HTTP → HTTPS**: Internal traffic is still plain HTTP

## Performance

- **Image size**: ~7MB (Alpine-based)
- **Memory**: Very low (Rust's zero-cost abstractions)
- **Throughput**: High (non-blocking async I/O)
- **Streaming**: No body buffer, forward directly

## See Also

- [Configuration](./CONFIGURATION.md)
- [Development](./DEVELOPMENT.md)
- [Troubleshooting](./TROUBLESHOOTING.md)
