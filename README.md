# HTTPS Reverse Proxy - [Docker Hub](https://hub.docker.com/r/manhpv151090/https)

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
  - [Running Locally](#running-locally-rust)
  - [Building Docker Image](#building-docker-image)
  - [Running Tests](#running-tests)
- [Certificates](#certificates)
  - [Auto-Generated CA Certificate](#auto-generated-ca-certificate)
  - [Trusting the CA Certificate](#trusting-the-ca-certificate)
  - [Uninstall CA](#uninstall-ca)
  - [Manual Import](#manual-import-alternative)
- [Troubleshooting](#troubleshooting)
- [License](#license)

## 🚀 Features

- ✅ **Port-based Routing**: Map specific ports to different backend services easily.
- ✅ **HTTPS Upstream**: Supports proxying to external HTTPS targets (e.g., public APIs).
- ✅ **WebSocket Support**: Full bidirectional WebSocket tunneling (`wss://` -> `ws://`).
- ✅ **Auto TLS**: Automatically generates self-signed CA and server certificates on startup.
- ✅ **Zero Config**: Works out-of-the-box with Docker Compose.
- ✅ **Streaming**: Non-buffering body forwarding for high performance.
- ✅ **Tiny Footprint**: Alpine-based Docker image (~7MB).

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
      - ./certs:/certs
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

| Variable   | Default            | Description                                                           |
| ---------- | ------------------ | --------------------------------------------------------------------- |
| `RUST_LOG` | `https_proxy=info` | Logging level (supported: `error`, `warn`, `info`, `debug`, `trace`). |

## 💻 Development

### Project Structure

```
.
├── proxy/                    # HTTPS reverse proxy crate
│   ├── src/
│   │   ├── main.rs           # Entry point, server setup
│   │   ├── lib.rs            # Library exports
│   │   ├── config.rs         # YAML config loading
│   │   ├── proxy.rs          # Core proxy logic, WebSocket handling
│   │   └── tls.rs            # TLS configuration
│   ├── tests/
│   │   └── integration_test.rs
│   ├── entrypoint.sh         # Docker entrypoint (CA + cert generation)
│   └── Cargo.toml
├── manage-ca/                # CA certificate management CLI
│   ├── src/
│   │   └── main.rs           # CLI entry point + NSS/browser cert management
│   └── Cargo.toml
├── Cargo.toml                # Workspace manifest
├── Dockerfile                # Multi-stage Docker build
├── docker-compose.yml        # Example composition with demo services
├── routes.yaml               # Example routes config
└── LICENSE
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

### Running Tests

```bash
cargo test --all-features
```

## 🔐 Certificates

### Auto-Generated CA Certificate

By default, the container automatically generates a Certificate Authority (CA) and server certificates on startup. The CA certificate is stored in the `certs/ca/` directory.

**Important**: Make sure to mount the `certs` directory in your `docker-compose.yml`:

```yaml
volumes:
  - ./routes.yaml:/etc/proxy/routes.yaml:ro
  - ./certs:/certs
```

### Trusting the CA Certificate

To avoid browser security warnings, use the `manage-ca` CLI tool to install the CA certificate.

**Prerequisites:**

| OS      | Required Software                            |
| ------- | -------------------------------------------- |
| Linux   | `libnss3-tools` (for Chrome/Chromium NSS DB) |
| macOS   | None (uses built-in `security` command)      |
| Windows | None (uses built-in `certutil.exe`)          |

```bash
# Ubuntu/Debian
sudo apt install libnss3-tools

# Fedora/RHEL
sudo dnf install nss-tools

# Arch
sudo pacman -S nss
```

**Option 1: Download from GitHub Releases**

Linux:

```bash
curl -L -o manage-ca https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-linux-amd64
chmod +x manage-ca
sudo ./manage-ca install
```

macOS:

```bash
# Intel
curl -L -o manage-ca https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-macos-amd64
# Apple Silicon
curl -L -o manage-ca https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-macos-arm64

chmod +x manage-ca
sudo ./manage-ca install
```

Windows (PowerShell as Administrator):

```powershell
Invoke-WebRequest -Uri "https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-windows-amd64.exe" -OutFile manage-ca.exe
.\manage-ca.exe install
```

**Option 2: Build from source**

```bash
cargo build --release -p manage-ca
sudo ./target/release/manage-ca install     # Linux/macOS
.\target\release\manage-ca.exe install      # Windows (as Admin)
```

**Custom cert path:**

```bash
sudo ./manage-ca install --cert /path/to/ca.crt
```

After installation, restart your browsers.

### Uninstall CA

```bash
sudo ./manage-ca uninstall          # Linux/macOS
.\manage-ca.exe uninstall           # Windows (as Admin)
```

### Manual Import (alternative)

If you prefer not to use `manage-ca`, you can import the certificate manually.

**Linux (Ubuntu/Debian):**

```bash
sudo cp certs/ca/ca.crt /usr/local/share/ca-certificates/local-dev-ca.crt
sudo update-ca-certificates
```

**Linux (Fedora/RHEL/Arch):**

```bash
sudo cp certs/ca/ca.crt /etc/pki/ca-trust/source/anchors/local-dev-ca.crt
sudo update-ca-trust extract
```

**Chrome/Chromium/Edge:**

1. Go to `chrome://certificate-manager`
2. Under **Custom**, click **Installed by you**
3. In **Trusted Certificates** section, click **Import**
4. Select `certs/ca/ca.crt`

**Firefox:**

1. Go to `about:preferences#privacy`
2. Scroll to **Certificates** → **View Certificates** → **Authorities** → **Import**
3. Select `certs/ca/ca.crt`
4. Check ✓ "Trust this CA to identify websites"

**macOS (manual):**

```bash
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain certs/ca/ca.crt
```

**Windows (manual):**

```powershell
certutil -addstore Root certs\ca\ca.crt
```

## ❓ Troubleshooting

**Port Conflicts:**
If a port is already in use on your host, change the mapping in `docker-compose.yml` (e.g., `"8443:443"` maps host port 8443 to container port 443).

**Certificate Errors:**
Since self-signed certificates are used by default, browsers and tools like `curl` will warn about security.

- **Browser**: Accept the security risk (usually under "Advanced").
- **curl**: Use the `-k` or `--insecure` flag.

## 📄 License

This project is open source and available under the [MIT License](LICENSE).
