# Build stage - using Alpine for smaller image
FROM rust:1.93.0-alpine3.23 AS builder

# Install build dependencies for native libs
RUN apk add --no-cache musl-dev

WORKDIR /app

# Copy workspace manifest
COPY Cargo.toml Cargo.lock* ./

# Copy crate manifests
COPY proxy/Cargo.toml proxy/Cargo.toml
COPY manage-ca/Cargo.toml manage-ca/Cargo.toml

# Create dummy src to cache dependencies
RUN mkdir -p proxy/src && echo "fn main() {}" > proxy/src/main.rs
RUN mkdir -p manage-ca/src && echo "fn main() {}" > manage-ca/src/main.rs

# Build dependencies (only proxy crate)
RUN cargo build --release --locked -p https-proxy
RUN rm proxy/src/*.rs

# Copy actual source code
COPY proxy/src ./proxy/src

RUN cargo build --release --locked -p https-proxy

RUN strip target/release/https-proxy || true

# Runtime stage - using Alpine for smaller image
FROM alpine:3.23

# Install CA certificates, openssl for generating self-signed certs
RUN apk add --no-cache ca-certificates openssl

# Create non-root user
RUN adduser -D -s /bin/false proxy

# Create directories for certs and config
RUN mkdir -p /certs /etc/proxy

# Copy binary
COPY --from=builder /app/target/release/https-proxy /usr/local/bin/https-proxy

# Copy entrypoint script
COPY proxy/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Set ownership
RUN chown -R proxy:proxy /certs /etc/proxy

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]

# Default command
CMD ["https-proxy"]
