# Getting Started

## Requirements

- [Docker](https://docs.docker.com/get-docker/)
- [Docker Compose](https://docs.docker.com/compose/install/)

## Setup Steps

### 1. Create Routes Configuration File

Create a `routes.yaml` file to define your routing rules:

```yaml
listeners:
  - port: 440
    target: http://api:3000      # Local API service
  - port: 441
    target: http://app:3001       # Local Web App
  - port: 442
    target: https://httpbin.org  # External HTTPS service
  - port: 443
    target: http://ws-echo:8080  # WebSocket service
```

### 2. Update docker-compose.yml

Add the proxy service to your docker-compose:

```yaml
services:
  proxy:
    image: manhpv151090/https:latest
    platform: linux/amd64
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

## Install CA Certificate

To avoid browser security warnings, see detailed instructions at [Certificates](./CERTIFICATES.md).

Quick way:

```bash
# Linux
curl -L -o manage-ca https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-linux-amd64
chmod +x manage-ca
sudo ./manage-ca install

# macOS (Intel)
curl -L -o manage-ca https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-macos-amd64
# macOS (Apple Silicon)
curl -L -o manage-ca https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-macos-arm64
chmod +x manage-ca
sudo ./manage-ca install

# Windows (PowerShell as Administrator)
Invoke-WebRequest -Uri "https://github.com/manhpham90vn/https/releases/latest/download/manage-ca-windows-amd64.exe" -OutFile manage-ca.exe
.\manage-ca.exe install
```

After installation, restart your browsers.

## Full Example

See `docker-compose.yml` in the project for a complete example with demo services.
