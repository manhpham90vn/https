# Troubleshooting

## Port Conflicts

If a port is already in use on the host:

```bash
# Check which process is using the port
sudo lsof -i :443

# Change the mapping in docker-compose.yml
ports:
  - "8443:443"  # Host port 8443 -> Container port 443
```

## Certificate Errors

### Browser Shows "Not Secure" Warning

Cause: CA certificate is not trusted on the system.

Solution:

```bash
# Install CA certificate
sudo ./manage-ca install
```

Or accept the warning in the browser (Advanced → Accept Risk).

### curl SSL Error

```bash
# Use -k flag to skip SSL verification
curl -k https://localhost:440/

# Or specify CA certificate
curl --cacert certs/ca/ca.crt https://localhost:440/
```

### Certificate Expired

Server certificate is valid for 1 year, CA certificate is valid for 10 years. To regenerate:

```bash
# Remove old certificates (both CA and server)
rm -rf certs/ca certs/cert.pem certs/key.pem

# Restart container (will auto-generate new ones)
docker compose restart proxy
```

## Connection Refused

### Upstream Service Not Ready

Check if the service is running:

```bash
docker compose ps
docker compose logs api
```

### DNS Resolution in Docker

Make sure all services are on the same network:

```yaml
services:
  proxy:
    networks:
      - proxy-net
  api:
    networks:
      - proxy-net

networks:
  proxy-net:
    driver: bridge
```

## WebSocket Not Working

### Test Connection

```bash
# Install wscat
npm install -g wscat

# Test WebSocket
wscat -n -c wss://localhost:443/ws
```

### Upstream Doesn't Support WebSocket

Make sure the target service supports WebSocket and is listening on the correct path.

## Docker Issues

### Image Too Large

Make sure to use multi-stage build (already available in Dockerfile).

### Permission Denied on Certs

This error can occur if the `certs` directory doesn't exist on the host:

```bash
# Create certs directory before running container
mkdir -p certs/ca
```

## Logging

Increase log level to debug:

```yaml
environment:
  - RUST_LOG=debug    # Or trace for more details
```

View logs:

```bash
docker compose logs -f proxy
```

## manage-ca Issues

### Linux: certutil Not Found

```bash
# Ubuntu/Debian
sudo apt install libnss3-tools

# Fedora/RHEL
sudo dnf install nss-tools

# Arch
sudo pacman -S nss
```

### macOS: Permission Denied

```bash
# Run with sudo
sudo ./manage-ca install
```

### Windows: Access Denied

Run PowerShell as Administrator.

## See Also

- [Getting Started](./GETTING_STARTED.md)
- [Configuration](./CONFIGURATION.md)
- [Architecture](./ARCHITECTURE.md)
