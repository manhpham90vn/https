# Build stage - using Alpine for smaller image
FROM rust:alpine AS builder

# Install build dependencies for native libs
RUN apk add --no-cache musl-dev

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release
RUN rm src/*.rs

# Copy actual source code
COPY src ./src

# Build actual binary (touch to invalidate cache)
RUN touch src/main.rs
RUN cargo build --release

# Runtime stage - using Alpine for smaller image
FROM alpine:latest

# Install CA certificates and openssl for generating self-signed certs
RUN apk add --no-cache ca-certificates openssl

# Create non-root user
RUN adduser -D -s /bin/false proxy

# Copy binary
COPY --from=builder /app/target/release/https-proxy /usr/local/bin/https-proxy

# Create directories for certs and config
RUN mkdir -p /certs /etc/proxy

# Generate self-signed certificate
RUN openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout /certs/key.pem \
    -out /certs/cert.pem \
    -subj "/C=VN/ST=Local/L=Local/O=Dev/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,DNS:*.localhost,IP:127.0.0.1"

# Set ownership
RUN chown -R proxy:proxy /certs /etc/proxy

# Switch to non-root user
USER proxy

# Default command
CMD ["https-proxy"]
