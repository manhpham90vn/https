# Build stage - using Alpine for smaller image
FROM rust:1.93.0-alpine3.23 AS builder

# Install build dependencies for native libs
RUN apk add --no-cache musl-dev

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release --locked
RUN rm src/*.rs

# Copy actual source code
COPY src ./src

RUN cargo build --release --locked

RUN strip target/release/https-proxy || true

# Runtime stage - using Alpine for smaller image
FROM alpine:3.23

# Install CA certificates, openssl for generating self-signed certs, and su-exec to drop privileges
RUN apk add --no-cache ca-certificates openssl

# Create non-root user
RUN adduser -D -s /bin/false proxy

# Create directories for certs and config
RUN mkdir -p /certs /etc/proxy

# Copy binary
COPY --from=builder /app/target/release/https-proxy /usr/local/bin/https-proxy

# Copy entrypoint script
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Set ownership
RUN chown -R proxy:proxy /certs /etc/proxy

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]

# Default command
CMD ["https-proxy"]
