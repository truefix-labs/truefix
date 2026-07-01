//! Build rustls configurations from a [`TlsSpec`] — TLS/mTLS driven entirely by configuration
//! (FR-017), rather than requiring callers to construct `rustls::{Server,Client}Config` in code.
//!
//! [`TlsSpec`]/[`TlsVersion`] are defined in `truefix-config` (the settings-mapping layer) and
//! re-exported here; this module is the mechanism that consumes them.

use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::WebPkiClientVerifier;
use rustls::{ClientConfig, RootCertStore, ServerConfig};

pub use truefix_config::{TlsSpec, TlsVersion};

/// An error building a TLS configuration from a [`TlsSpec`].
#[derive(Debug, thiserror::Error)]
pub enum TlsConfigError {
    /// A referenced PEM file could not be read or parsed.
    #[error("reading {path}: {source}")]
    Pem {
        /// The file path that could not be read.
        path: String,
        /// The underlying PEM/I/O error.
        #[source]
        source: rustls::pki_types::pem::Error,
    },
    /// No certificate was found in the key-store PEM.
    #[error("no certificate found in {0}")]
    NoCertificate(String),
    /// No private key was found in the key-store PEM.
    #[error("no private key found in {0}")]
    NoPrivateKey(String),
    /// `NeedClientAuth`/mTLS was requested without a trust store to verify against.
    #[error("NeedClientAuth requires a trust store (SocketTrustStore)")]
    MissingTrustStoreForClientAuth,
    /// The client-cert verifier could not be built.
    #[error("client cert verifier: {0}")]
    ClientVerifier(#[from] rustls::server::VerifierBuilderError),
    /// The rustls configuration could not be built.
    #[error("rustls: {0}")]
    Rustls(#[from] rustls::Error),
}

fn pem_err(path: &Path, source: rustls::pki_types::pem::Error) -> TlsConfigError {
    TlsConfigError::Pem {
        path: path.display().to_string(),
        source,
    }
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, TlsConfigError> {
    CertificateDer::pem_file_iter(path)
        .map_err(|source| pem_err(path, source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| pem_err(path, source))
}

fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, TlsConfigError> {
    match PrivateKeyDer::from_pem_file(path) {
        Ok(key) => Ok(key),
        Err(rustls::pki_types::pem::Error::NoItemsFound) => {
            Err(TlsConfigError::NoPrivateKey(path.display().to_string()))
        }
        Err(source) => Err(pem_err(path, source)),
    }
}

fn load_root_store(path: &Path) -> Result<RootCertStore, TlsConfigError> {
    let mut roots = RootCertStore::empty();
    for cert in load_certs(path)? {
        // Best-effort: a malformed/duplicate root is skipped rather than aborting the whole store.
        let _ = roots.add(cert);
    }
    Ok(roots)
}

const TLS13_ONLY: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];
const TLS12_AND_UP: &[&rustls::SupportedProtocolVersion] =
    &[&rustls::version::TLS12, &rustls::version::TLS13];

fn protocol_versions(
    min: Option<TlsVersion>,
) -> &'static [&'static rustls::SupportedProtocolVersion] {
    match min {
        Some(TlsVersion::Tls13) => TLS13_ONLY,
        Some(TlsVersion::Tls12) | None => TLS12_AND_UP,
    }
}

/// Build a server-side (acceptor) TLS configuration from `spec` (FR-017), including mTLS when
/// `need_client_auth` is set.
pub fn build_server_config(spec: &TlsSpec) -> Result<Arc<ServerConfig>, TlsConfigError> {
    let certs = load_certs(&spec.key_store_path)?;
    if certs.is_empty() {
        return Err(TlsConfigError::NoCertificate(
            spec.key_store_path.display().to_string(),
        ));
    }
    let key = load_private_key(&spec.key_store_path)?;
    let builder = ServerConfig::builder_with_protocol_versions(protocol_versions(spec.min_version));

    let config = if spec.need_client_auth {
        let trust_path = spec
            .trust_store_path
            .as_deref()
            .ok_or(TlsConfigError::MissingTrustStoreForClientAuth)?;
        let roots = Arc::new(load_root_store(trust_path)?);
        let verifier = WebPkiClientVerifier::builder(roots).build()?;
        builder
            .with_client_cert_verifier(verifier)
            .with_single_cert(certs, key)?
    } else {
        builder.with_no_client_auth().with_single_cert(certs, key)?
    };
    Ok(Arc::new(config))
}

/// Build a client-side (initiator) TLS configuration from `spec` (FR-017). When
/// `need_client_auth` is set, the initiator also presents `key_store_path` as its own client
/// certificate (mTLS).
pub fn build_client_config(spec: &TlsSpec) -> Result<Arc<ClientConfig>, TlsConfigError> {
    let roots = match &spec.trust_store_path {
        Some(p) => load_root_store(p)?,
        None => RootCertStore::empty(),
    };
    let builder = ClientConfig::builder_with_protocol_versions(protocol_versions(spec.min_version))
        .with_root_certificates(roots);

    let config = if spec.need_client_auth {
        let certs = load_certs(&spec.key_store_path)?;
        let key = load_private_key(&spec.key_store_path)?;
        builder.with_client_auth_cert(certs, key)?
    } else {
        builder.with_no_client_auth()
    };
    Ok(Arc::new(config))
}
