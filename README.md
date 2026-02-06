# HTTPS Reverse Proxy - [Docker hub](https://hub.docker.com/r/manhpv151090/https)

A lightweight HTTPS reverse proxy written in Rust, designed for local development with Docker Compose.

## Features

- ✅ **Port-based routing** - Each port maps to a different backend service
- ✅ **HTTPS Upstream** - Supports proxying to HTTPS targets (e.g., external APIs)
- ✅ **WebSocket Support** - Full bidirectional WebSocket tunneling (wss:// -> ws://)
- ✅ **Auto TLS** - Auto-generated self-signed certificates (rustls)
- ✅ **Zero Config** - Works out-of-the-box with Docker Compose
- ✅ **Streaming** - Non-buffering body forwarding
- ✅ **Tiny** - Alpine-based Docker image (~25MB)

## Quick Start

### 1. Configure Listeners

Edit `routes.yaml`:

```yaml
listeners:
  - port: 440
    target: http://api:3000
  - port: 441
    target: http://app:3001
  - port: 442
    target: https://httpbin.org
  - port: 443
    target: http://ws-echo:8080
```

### 2. Update docker-compose.yml

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

### 3. Run with Docker Compose

```bash
docker compose up --build
```

### 4. Test

**HTTPS Request:**

```bash
# Port 440 -> api service
curl -k https://localhost:440/

# Port 442 -> external HTTPS
curl -k https://localhost:442/
```

**WebSocket:**

```bash
# Install Node.js & wscat
npm install -g wscat

# Connect
wscat -n -c wss://localhost:443/ws
```

## Configuration

| Environment Variable | Default                  | Description           |
| -------------------- | ------------------------ | --------------------- |
| `CONFIG_PATH`        | `/etc/proxy/routes.yaml` | Path to routes config |
| `CERT_PATH`          | `/certs/cert.pem`        | TLS certificate       |
| `KEY_PATH`           | `/certs/key.pem`         | TLS private key       |
| `RUST_LOG`           | `https_proxy=info`       | Log level             |

## Project Structure

```
https/
├── src/
│   ├── main.rs      # Entry point, multi-port TLS setup
│   ├── config.rs    # YAML config loader
│   └── proxy.rs     # Request forwarding logic (HTTP + WebSocket)
├── routes.yaml      # Listener configuration
├── Dockerfile       # Multi-stage Alpine build
└── docker-compose.yml
```

## Custom Certificates (Optional)

If you prefer to use your own certificates instead of auto-generated ones:

**Using mkcert (recommended for local dev):**

```bash
# Install mkcert
brew install mkcert        # macOS
sudo apt install mkcert    # Ubuntu

# Install local CA
mkcert -install

# Generate certificates
mkdir -p certs
mkcert -key-file certs/key.pem -cert-file certs/cert.pem localhost 127.0.0.1
```

Then mount them in `docker-compose.yml`:

```yaml
volumes:
  - ./certs:/certs:ro
```

## License

MIT
