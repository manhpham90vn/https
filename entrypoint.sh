#!/bin/sh
set -e

# Generate self-signed certificate if it doesn't exist
if [ ! -f /certs/cert.pem ] || [ ! -f /certs/key.pem ]; then
    echo "Generating self-signed certificate..."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout /certs/key.pem \
        -out /certs/cert.pem \
        -subj "/C=VN/ST=Local/L=Local/O=Dev/CN=localhost" \
        -addext "subjectAltName=DNS:localhost,DNS:*.localhost,IP:127.0.0.1"
    
    # Ensure ownership is correct
    chown proxy:proxy /certs/cert.pem /certs/key.pem
fi

# Switch to proxy user and execute the application
echo "Starting application..."
exec "$@"
