#!/bin/sh
set -e

# Create CA directory if it doesn't exist
mkdir -p /certs/ca

# Step 1: Generate Root CA if it doesn't exist
if [ ! -f /certs/ca/ca.key ] || [ ! -f /certs/ca/ca.crt ]; then
    echo "=== Creating Local Certificate Authority ==="
    
    # Generate CA private key
    openssl genrsa -out /certs/ca/ca.key 4096
    
    # Create OpenSSL config for CA
    cat > /certs/ca.cnf << EOF
[req]
distinguished_name = req_distinguished_name
x509_extensions = v3_ca
prompt = no

[req_distinguished_name]
C = VN
ST = Local
L = Local
O = Local Dev CA
CN = Local Development CA

[v3_ca]
basicConstraints = critical,CA:TRUE
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash
EOF
    
    # Generate CA certificate (valid for 10 years)
    openssl req -x509 -new -nodes \
        -key /certs/ca/ca.key \
        -sha256 -days 3650 \
        -out /certs/ca/ca.crt \
        -config /certs/ca.cnf
    
    # Clean up
    rm -f /certs/ca.cnf
    
    echo "✓ Root CA created successfully"
fi

# Step 2: Install CA certificate into system trust store
if [ ! -f /usr/local/share/ca-certificates/local-dev-ca.crt ]; then
    echo "=== Installing CA into system trust store ==="
    
    # Copy CA cert to system trust store
    cp /certs/ca/ca.crt /usr/local/share/ca-certificates/local-dev-ca.crt
    
    # Update CA certificates
    update-ca-certificates
    
    echo "✓ CA installed into trust store"
fi

# Step 3: Generate server certificate signed by CA
if [ ! -f /certs/cert.pem ] || [ ! -f /certs/key.pem ]; then
    echo "=== Generating server certificate signed by CA ==="
    
    # Generate server private key
    openssl genrsa -out /certs/key.pem 2048
    
    # Create certificate signing request (CSR)
    openssl req -new \
        -key /certs/key.pem \
        -out /certs/server.csr \
        -subj "/C=VN/ST=Local/L=Local/O=Dev/CN=localhost"
    
    # Create extension file for SAN (Subject Alternative Names)
    cat > /certs/server.ext << EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = *.localhost
DNS.3 = *.local
IP.1 = 127.0.0.1
IP.2 = ::1
EOF
    
    # Sign the certificate with CA (valid for 1 year)
    openssl x509 -req \
        -in /certs/server.csr \
        -CA /certs/ca/ca.crt \
        -CAkey /certs/ca/ca.key \
        -CAcreateserial \
        -out /certs/cert.pem \
        -days 365 \
        -sha256 \
        -extfile /certs/server.ext
    
    # Clean up temporary files
    rm -f /certs/server.csr /certs/server.ext
    
    echo "✓ Server certificate created and signed by CA"
fi

# Ensure ownership is correct
chown -R proxy:proxy /certs

# Display certificate information
echo ""
echo "=== Certificate Information ==="
echo "CA Certificate: /certs/ca/ca.crt"
echo "Server Certificate: /certs/cert.pem"
echo "Server Key: /certs/key.pem"
echo ""
echo "To trust this CA on your host machine, run:"
echo "  Linux:   ./install-ca.sh"
echo "  macOS:   ./install-ca.sh"
echo "  Windows: ./install-ca.ps1"
echo ""

# Switch to proxy user and execute the application
echo "Starting application..."
exec "$@"
