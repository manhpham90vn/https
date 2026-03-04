# Configuration

## Routes Configuration

The `routes.yaml` file defines the routes for the proxy. Basic structure:

```yaml
listeners:
  - port: <LISTENING_PORT>
    target: <UPSTREAM_URL>
```

### Properties

| Property | Required | Description |
|----------|----------|-------------|
| `port` | Yes | Port on the container that accepts HTTPS connections |
| `target` | Yes | Upstream URL where requests will be forwarded |

### Supported Protocols

The `target` property supports:

- `http://` - HTTP backend
- `https://` - HTTPS backend (external APIs)
- `ws://` - WebSocket backend
- `wss://` - WebSocket Secure backend

### Configuration Examples

```yaml
listeners:
  # HTTP backend
  - port: 440
    target: http://api:3000

  # External HTTPS API
  - port: 441
    target: https://api.stripe.com

  # WebSocket service
  - port: 442
    target: ws://chat-server:8080

  # WebSocket Secure
  - port: 443
    target: wss://secure-ws.example.com
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `https_proxy=info,tower_http=debug` | Logging level (supported: `error`, `warn`, `info`, `debug`, `trace`) |

### Usage Example

```yaml
services:
  proxy:
    image: manhpv151090/https:latest
    environment:
      - RUST_LOG=debug
    # ...
```

## Advanced Configuration

### Mount Certificates Directory

Make sure to mount the `certs` directory to store CA certificates:

```yaml
volumes:
  - ./routes.yaml:/etc/proxy/routes.yaml:ro
  - ./certs:/certs
```

### Port Mapping

If a port is already in use on the host, change the mapping in docker-compose:

```yaml
ports:
  - "8443:443"  # Host port 8443 -> Container port 443
```

## See Also

- [Architecture](./ARCHITECTURE.md)
- [Development](./DEVELOPMENT.md)
