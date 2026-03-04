# Local Development

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (latest version recommended)
- Docker & Docker Compose (for production build)

## Project Structure

```
.
├── proxy/                    # HTTPS reverse proxy crate
│   ├── src/
│   │   ├── main.rs           # Entry point, server setup
│   │   ├── lib.rs            # Library exports
│   │   ├── config.rs         # YAML config loading
│   │   ├── proxy.rs          # Core proxy logic, WebSocket handling
│   │   └── tls.rs            # TLS configuration (insecure client)
│   ├── tests/
│   │   └── integration_test.rs
│   └── Cargo.toml
├── manage-ca/                # CA certificate management CLI
│   ├── src/
│   │   └── main.rs           # CLI entry point + NSS/browser cert management
│   └── Cargo.toml
├── Cargo.toml                # Workspace manifest
├── Dockerfile                # Multi-stage Docker build
├── entrypoint.sh             # Docker entrypoint (CA + cert generation)
├── docker-compose.yml        # Example composition with demo services
├── routes.yaml               # Example routes config
└── LICENSE
```

## Running Directly with Rust

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Run the Application

**Note**: Certificates are automatically created by `entrypoint.sh` when the container starts. When running directly with Rust, create the `certs` directory first:

```bash
# Create certs directory (to store certificates)
mkdir -p certs/ca
```

```bash
# Run with release profile (faster)
cargo run --release
```

### Run Tests

```bash
cargo test --all-features
```

## Build Docker Image

### Build Locally

```bash
docker build -t my-https-proxy .
```

### Build with Docker Compose

```bash
docker compose build
```

## CA Management Tool (manage-ca)

### Build from Source

```bash
cargo build --release -p manage-ca
```

### Install CA Certificate

```bash
# Linux/macOS
sudo ./target/release/manage-ca install

# Windows (as Administrator)
.\target\release\manage-ca.exe install
```

### Uninstall CA

```bash
# Linux/macOS
sudo ./target/release/manage-ca uninstall

# Windows (as Administrator)
.\target\release\manage-ca.exe uninstall
```

## Development Environment

### VS Code

Recommended extensions:
- rust-analyzer
- Even Better TOML (for YAML/TOML)

### Logging

Adjust log level via environment variable:

```bash
RUST_LOG=debug cargo run --release
```

Levels: `error`, `warn`, `info`, `debug`, `trace`

## See Also

- [Configuration](./CONFIGURATION.md)
- [Architecture](./ARCHITECTURE.md)
- [Troubleshooting](./TROUBLESHOOTING.md)
