# Certificates

## Overview

The proxy uses TLS certificates to provide HTTPS connections. Certificates are automatically generated when the container starts.

## Auto-Generated Certificates

### CA Certificate

Certificate Authority (CA) is created and stored in `certs/ca/`:

- **Location**: `/certs/ca/ca.crt` and `/certs/ca/ca.key`
- **Validity**: 10 years
- **Type**: Self-signed Root CA

### Server Certificate

Server certificate is signed by CA and stored in `certs/`:

- **Location**: `/certs/cert.pem` and `/certs/key.pem`
- **Validity**: 1 year
- **SANs**: localhost, \*.localhost, \*.local, 127.0.0.1, ::1

## Install CA Certificate

To avoid browser security warnings, install the CA to the system trust store.

### Requirements

| OS | Required Software |
| --- | --- |
| Linux | `libnss3-tools` (for Chrome/Chromium NSS DB) |
| macOS | None (uses built-in `security` command) |
| Windows | None (uses built-in `certutil.exe`) |

### Install Tools

```bash
# Ubuntu/Debian
sudo apt install libnss3-tools

# Fedora/RHEL
sudo dnf install nss-tools

# Arch
sudo pacman -S nss
```

### Download and Install manage-ca

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

**Option 2: Build from Source**

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

## Uninstall CA

```bash
sudo ./manage-ca uninstall          # Linux/macOS
.\manage-ca.exe uninstall           # Windows (as Admin)
```

## Manual Import

If you prefer not to use `manage-ca`, you can import manually:

### Linux (Ubuntu/Debian):

```bash
sudo cp certs/ca/ca.crt /usr/local/share/ca-certificates/local-dev-ca.crt
sudo update-ca-certificates
```

### Linux (Fedora/RHEL/Arch):

```bash
sudo cp certs/ca/ca.crt /etc/pki/ca-trust/source/anchors/local-dev-ca.crt
sudo update-ca-trust extract
```

### Chrome/Chromium/Edge:

1. Go to `chrome://certificate-manager`
2. Under **Custom**, click **Installed by you**
3. In **Trusted Certificates** section, click **Import**
4. Select `certs/ca/ca.crt`

### Firefox:

1. Go to `about:preferences#privacy`
2. Scroll to **Certificates** → **View Certificates** → **Authorities** → **Import**
3. Select `certs/ca/ca.crt`
4. Check ✓ "Trust this CA to identify websites"

### macOS:

```bash
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain certs/ca/ca.crt
```

### Windows:

```powershell
certutil -addstore Root certs\ca\ca.crt
```

## Certificate Paths in Container

| Type | Path in Container |
|------|------------------|
| CA Certificate | `/certs/ca/ca.crt` |
| CA Key | `/certs/ca/ca.key` |
| Server Certificate | `/certs/cert.pem` |
| Server Key | `/certs/key.pem` |

## Mount Certificates Directory

Make sure to mount the `certs` directory in docker-compose:

```yaml
volumes:
  - ./routes.yaml:/etc/proxy/routes.yaml:ro
  - ./certs:/certs
```

## See Also

- [Getting Started](./GETTING_STARTED.md)
- [Configuration](./CONFIGURATION.md)
- [Troubleshooting](./TROUBLESHOOTING.md)
