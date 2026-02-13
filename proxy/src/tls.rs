use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use std::sync::Arc;

/// A server certificate verifier that accepts any certificate.
///
/// # Security Warning
/// This provides NO security against MitM attacks. It should only be used
/// for local development against self-signed certificates.
#[derive(Debug)]
pub struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// Create a ClientConfig that trusts all certificates
pub fn get_insecure_client_config() -> ClientConfig {
    let mut config = ClientConfig::builder()
        .with_root_certificates(RootCertStore::empty())
        .with_no_client_auth();

    // Dangerous configuration: disable server certificate verification
    config
        .dangerous()
        .set_certificate_verifier(Arc::new(NoCertificateVerification));

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_insecure_client_config_creates_config() {
        // Install default crypto provider (required for rustls 0.23+)
        let _ = rustls::crypto::ring::default_provider().install_default();

        let config = get_insecure_client_config();
        // Verify config is created successfully with ALPN protocols
        assert!(config.alpn_protocols.is_empty()); // Default is empty
    }

    #[test]
    fn test_no_certificate_verification_supported_schemes() {
        let verifier = NoCertificateVerification;
        let schemes = verifier.supported_verify_schemes();

        assert!(schemes.contains(&SignatureScheme::RSA_PKCS1_SHA256));
        assert!(schemes.contains(&SignatureScheme::ECDSA_NISTP256_SHA256));
        assert!(schemes.contains(&SignatureScheme::ED25519));
        assert_eq!(schemes.len(), 10);
    }

    #[test]
    fn test_no_certificate_verification_verify_server_cert() {
        use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

        let verifier = NoCertificateVerification;
        let cert = CertificateDer::from(vec![0u8; 32]);
        let server_name = ServerName::try_from("example.com").unwrap();

        let result = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());

        assert!(result.is_ok());
    }
}
