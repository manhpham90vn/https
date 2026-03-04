# HTTPS Reverse Proxy

![Docker Image Size (tag)](https://img.shields.io/docker/image-size/manhpv151090/https/latest)
![Docker Pulls](https://img.shields.io/docker/pulls/manhpv151090/https)
![License](https://img.shields.io/github/license/manhpham90vn/https)

## Introduction

A lightweight, high-performance HTTPS reverse proxy written in Rust. Designed for local development with Docker Compose to easily route traffic to multiple backend services with automatic self-signed TLS certificates.

---

## Table of Contents

- [Introduction](#introduction)
- [Features](#features)
- [Problem & Solution](#problem--solution)
- [Use Cases](#use-cases)
- [Quick Start](./docs/GETTING_STARTED.md)
- [Configuration](./docs/CONFIGURATION.md)
- [Certificates](./docs/CERTIFICATES.md)
- [Development](./docs/DEVELOPMENT.md)
- [Architecture](./docs/ARCHITECTURE.md)
- [Troubleshooting](./docs/TROUBLESHOOTING.md)

---

## Features

- ✅ **Port-based Routing**: Map specific ports to different backend services easily.
- ✅ **HTTPS Upstream**: Supports proxying to external HTTPS targets (e.g., public APIs).
- ✅ **WebSocket Support**: Full bidirectional WebSocket tunneling (`wss://` -> `ws://`).
- ✅ **Auto TLS**: Automatically generates self-signed CA and server certificates on startup.
- ✅ **Zero Config**: Works out-of-the-box with Docker Compose.
- ✅ **Streaming**: Non-buffering body forwarding for high performance.
- ✅ **Tiny Footprint**: Alpine-based Docker image (~7MB).

---

## Problem & Solution

### ❌ Before This Tool

When developing web applications locally, developers often face these issues:

1. **Cannot test HTTPS locally**
   - Modern browsers block many features without HTTPS (Service Workers, Push Notifications, Geolocation API)
   - Some libraries/frameworks only work with HTTPS
   - PWAs cannot be installed over HTTP

2. **Certificate management is complex**
   - Creating self-signed certs with OpenSSL is difficult and error-prone
   - Each service requires separate configuration, inconsistent
   - Difficulty trusting certificates in browsers

3. **Cannot test with external HTTPS services**
   - Cannot proxy from local HTTPS to external HTTPS API
   - Hard to test features related to SSL/TLS handshake

4. **WebSocket over HTTPS doesn't work**
   - `wss://` (WebSocket Secure) doesn't work with HTTP backend
   - Missing ability to tunnel WebSocket over HTTPS

5. **Complex multi-port/services setup**
   - No centralized proxy for multiple services
   - Difficult to manage configuration with many microservices

### ✅ Solution with HTTPS Proxy

This tool solves all the above issues:

| Problem | Solution |
|---------|----------|
| Test HTTPS locally | Auto-generate TLS certificates, no config needed |
| Certificate management | Auto-generate CA and server certs, easy to trust via CLI |
| Proxy to external HTTPS | Supports upstream `https://` targets |
| WebSocket over HTTPS | Native WebSocket tunneling (`wss://` → `ws://`) |
| Multiple services | Port-based routing, defined in `routes.yaml` |
| Docker integration | Works seamlessly with Docker Compose |

---

## Use Cases

This tool is useful for the following scenarios:

### 1. PWA Development (Progressive Web App)

```yaml
listeners:
  - port: 443
    target: http://localhost:3000
```

- Test Service Workers, Push Notifications
- Install PWA on desktop/mobile
- Use restricted Web APIs (Geolocation, Device Orientation)

### 2. Multi-service Development

```yaml
listeners:
  - port: 440
    target: http://api:3000      # API Gateway
  - port: 441
    target: http://web:3001      # Frontend
  - port: 442
    target: http://admin:3002    # Admin Panel
```

- Run multiple services simultaneously
- All have HTTPS with fake domain (`localhost`)
- Centralized access point

### 3. Proxy to External APIs

```yaml
listeners:
  - port: 8443
    target: https://api.stripe.com
  - port: 8444
    target: https://jsonplaceholder.typicode.com
```

- Test webhooks from external services (Stripe, PayPal)
- Debug HTTPS requests to external APIs
- Bypass CORS in development

### 4. WebSocket Development

```yaml
listeners:
  - port: 443
    target: ws://localhost:8080
```

- Real-time apps (chat, notifications, collaborative editing)
- Game servers
- Live dashboards

### 5. Testing Third-party Integrations

```yaml
listeners:
  - port: 8445
    target: https://webhook.site
```

- Test webhook payloads
- Debug OAuth flows
- Verify API responses

---

## Quick Start

See [Getting Started](./docs/GETTING_STARTED.md) to get started quickly.

```bash
# Quick start with Docker Compose
docker compose up --build
```

---

## License

MIT License - See [LICENSE](./LICENSE) for details.

---

**Docker Hub**: https://hub.docker.com/r/manhpv151090/https
