# HTTPS Reverse Proxy

![Docker Image Size (tag)](https://img.shields.io/docker/image-size/manhpv151090/https/latest)
![Docker Pulls](https://img.shields.io/docker/pulls/manhpv151090/https)
![License](https://img.shields.io/github/license/manhpham90vn/https)

A lightweight, high-performance HTTPS reverse proxy written in Rust. Designed for local development with Docker Compose to easily route traffic to multiple backend services with automatic self-signed TLS certificates.

---

## 📚 Table of Contents

- [Features](#features)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
  - [Routes Configuration](#routes-configuration)
  - [Environment Variables](#environment-variables)
- [Development](#development)
  - [Project Structure](#project-structure)
  - [Running Tests](#running-tests)
- [Custom Certificates](#custom-certificates)
- [Troubleshooting](#troubleshooting)
- [License](#license)

## 🚀 Features

- ✅ **Port-based Routing**: Map specific ports to different backend services easily.
- ✅ **HTTPS Upstream**: Supports proxying to external HTTPS targets (e.g., public APIs).
- ✅ **WebSocket Support**: Full bidirectional WebSocket tunneling (`wss://` -> `ws://`).
- ✅ **Auto TLS**: Automatically generates self-signed certificates using `rustls` on startup.
- ✅ **Zero Config**: Works out-of-the-box with Docker Compose.
- ✅ **Streaming**: Non-buffering body forwarding for high performance.
- ✅ **Tiny Footprint**: Alpine-based Docker image (~25MB).

## 🛠 Prerequisites

- [Docker](https://docs.docker.com/get-docker/)
- [Docker Compose](https://docs.docker.com/compose/install/)

## ⚡ Quick Start

### 1. Configure Listeners

Create a `routes.yaml` file to define your routing rules:

```yaml
listeners:
  - port: 440
    target: http://api:3000 # Local API service
  - port: 441
    target: http://app:3001 # Local Web App
  - port: 442
    target: https://httpbin.org # External HTTPS service
  - port: 443
    target: http://ws-echo:8080 # WebSocket service
```

### 2. Update `docker-compose.yml`

Add the proxy service to your composition:

```yaml
services:
  proxy:
    image: manhpv151090/https:latest
    ports:
      - "440:440"
      - "441:441"
      - "442:442"
      - "443:443"
    volumes:
      - ./routes.yaml:/etc/proxy/routes.yaml:ro
```

### 3. Run the Proxy

```bash
docker compose up --build
```

### 4. Verify

- **HTTPS Request**:
  ```bash
  curl -k https://localhost:440/
  ```
- **WebSocket Connection**:
  ```bash
  wscat -n -c wss://localhost:443/ws
  ```

## ⚙️ Configuration

### Routes Configuration

The `routes.yaml` file supports the following structure:

```yaml
listeners:
  - port: <LISTENING_PORT>
    target: <UPSTREAM_URL>
```

- **port**: The port on the proxy container that will accept incoming HTTPS connections.
- **target**: The upstream URL where requests will be forwarded. Supports `http://`, `https://`, and `ws://`.

### Environment Variables

| Variable      | Default                  | Description                                                           |
| ------------- | ------------------------ | --------------------------------------------------------------------- |
| `CONFIG_PATH` | `/etc/proxy/routes.yaml` | Path to the routes configuration file inside the container.           |
| `CERT_PATH`   | `/certs/cert.pem`        | Path to the SSL certificate file.                                     |
| `KEY_PATH`    | `/certs/key.pem`         | Path to the SSL private key file.                                     |
| `RUST_LOG`    | `https_proxy=info`       | Logging level (supported: `error`, `warn`, `info`, `debug`, `trace`). |

## 💻 Development

### Project Structure

```
.
├── src/
│   ├── main.rs       # Entry point, server setup
│   ├── lib.rs        # Library exports
│   ├── config.rs     # YAML config loading
│   ├── proxy.rs      # Core proxy logic, WebSocket handling
│   └── tls.rs        # TLS configuration
├── tests/
│   └── integration_test.rs  # Integration tests
├── routes.yaml       # Example routes config
├── Dockerfile        # Multi-stage Docker build
└── docker-compose.yml
```

### Running Locally (Rust)

If you have Rust installed, you can run the project natively:

1.  **Install Rust**:
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```
2.  **Run**:
    ```bash
    cargo run --release
    ```

### Building Docker Image

To build the Docker image locally:

```bash
docker build -t my-https-proxy .
```

## 🔐 Custom Certificates

By default, the container generates self-signed certificates on startup. To use your own trusted certificates (e.g., generated with `mkcert`):

1.  **Generate Certificates** (using `mkcert`):

    ```bash
    mkcert -install
    mkdir -p certs
    mkcert -key-file certs/key.pem -cert-file certs/cert.pem localhost 127.0.0.1
    ```

2.  **Mount in Docker**:
    Update your `docker-compose.yml`:
    ```yaml
    volumes:
      - ./routes.yaml:/etc/proxy/routes.yaml:ro
      - ./certs:/certs:ro # Mount your custom certs directory
    ```
    _Note: The container checks for `/certs/cert.pem` and `/certs/key.pem` on startup._

## ❓ Troubleshooting

**Port Conflicts:**
If a port is already in use on your host, change the mapping in `docker-compose.yml` (e.g., `"8443:443"` maps host port 8443 to container port 443).

**Certificate Errors:**
Since self-signed certificates are used by default, browsers and tools like `curl` will warn about security.

- **Browser**: Accept the security risk (usually under "Advanced").
- **curl**: Use the `-k` or `--insecure` flag.

## 📄 License

This project is open source and available under the [MIT License](LICENSE).
