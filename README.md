# HTTPS Reverse Proxy

A lightweight HTTPS reverse proxy written in Rust, designed for local development with Docker Compose.

## Docker Hub

```yaml
# docker-compose.yml
services:
  proxy:
    image: manhpv151090/https:latest
    ports:
      - "440:440"
      - "441:441"
    volumes:
      - ./routes.yaml:/etc/proxy/routes.yaml:ro
```

## Features

- ✅ **Port-based routing** - Each port maps to a different backend service
- ✅ HTTPS with auto-generated self-signed certificates (rustls)
- ✅ Streaming body (no buffering)
- ✅ X-Forwarded-\* headers injection
- ✅ WebSocket proxying support
- ✅ Alpine-based Docker image (~15MB)

## Quick Start

### 1. Configure Listeners

Edit `routes.yaml`:

```yaml
listeners:
  - port: 440
    target: http://api:3000

  - port: 441
    target: http://app:3001
```

### 2. Update docker-compose.yml

```yaml
proxy:
  build: .
  ports:
    - "440:440"
    - "441:441"
```

### 3. Run with Docker Compose

```bash
docker compose up --build
```

> **Note:** SSL certificates are automatically generated during Docker build.

### 4. Test

```bash
# Port 440 -> api service
curl -k https://localhost:440/

# Port 441 -> app service
curl -k https://localhost:441/
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
│   └── proxy.rs     # Request forwarding logic
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
