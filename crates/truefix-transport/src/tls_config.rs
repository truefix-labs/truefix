//! Build rustls configurations from a [`TlsSpec`] — TLS/mTLS driven entirely by configuration
//! (FR-017), rather than requiring callers to construct `rustls::{Server,Client}Config` in code.
//!
//! [`TlsSpec`]/[`TlsVersion`] are defined in `truefix-config` (the settings-mapping layer) and
//! re-exported here; this module is the mechanism that consumes them.

use std::path::Path;
use std::sync::Arc;

use rustls::crypto::CryptoProvider;
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
    /// Inline PEM bytes (`SocketKeyStoreBytes`/`SocketTrustStoreBytes`) could not be parsed
    /// (FR-017).
    #[error("parsing inline PEM bytes: {0}")]
    PemBytes(#[source] rustls::pki_types::pem::Error),
    /// No certificate was found in the key-store PEM.
    #[error("no certificate found in {0}")]
    NoCertificate(String),
    /// No private key was found in the key-store PEM.
    #[error("no private key found in {0}")]
    NoPrivateKey(String),
    /// Neither `SocketKeyStore` (a path) nor `SocketKeyStoreBytes` (inline PEM) was provided.
    #[error("no SocketKeyStore path or SocketKeyStoreBytes was provided")]
    MissingKeyStore,
    /// `NeedClientAuth`/mTLS was requested without a trust store to verify against.
    #[error("NeedClientAuth requires a trust store (SocketTrustStore/SocketTrustStoreBytes)")]
    MissingTrustStoreForClientAuth,
    /// The client-cert verifier could not be built.
    #[error("client cert verifier: {0}")]
    ClientVerifier(#[from] rustls::server::VerifierBuilderError),
    /// The rustls configuration could not be built.
    #[error("rustls: {0}")]
    Rustls(#[from] rustls::Error),
    /// Names in `CipherSuites` (FR-017) that do not match any suite supported by the
    /// process-default provider.
    #[error("CipherSuites contains unrecognized cipher suites: {0:?}")]
    UnrecognizedCipherSuites(Vec<String>),
    /// T166/T167 (feature 009, NEW-95): a configured trust store (`SocketTrustStore`/
    /// `SocketTrustStoreBytes`) produced zero usable trust anchors — every certificate present
    /// either failed to parse/add, or the source had none at all. Previously silent (`let _ =
    /// roots.add(cert);`), so a fully-broken trust-store file produced an empty, non-functional
    /// `RootCertStore` with no diagnostic; every subsequent handshake would then fail with no clue
    /// why.
    #[error("trust store {0:?} produced no usable certificates")]
    EmptyTrustStore(String),
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

/// The key-store's certificate chain, from inline bytes when set (`SocketKeyStoreBytes`,
/// FR-017), otherwise from `SocketKeyStore`'s file path. Exactly one of the two must be set — the
/// resolver (`resolve_tls`) already enforces this, but a direct `TlsSpec` construction is checked
/// here too via [`TlsConfigError::MissingKeyStore`].
fn key_store_certs(spec: &TlsSpec) -> Result<Vec<CertificateDer<'static>>, TlsConfigError> {
    if let Some(bytes) = &spec.key_store_bytes {
        return CertificateDer::pem_slice_iter(bytes)
            .collect::<Result<Vec<_>, _>>()
            .map_err(TlsConfigError::PemBytes);
    }
    match &spec.key_store_path {
        Some(path) => load_certs(path),
        None => Err(TlsConfigError::MissingKeyStore),
    }
}

/// The key-store's private key, from inline bytes when set, otherwise from the file path. See
/// [`key_store_certs`] for the precedence rule.
fn key_store_private_key(spec: &TlsSpec) -> Result<PrivateKeyDer<'static>, TlsConfigError> {
    if let Some(bytes) = &spec.key_store_bytes {
        return match PrivateKeyDer::from_pem_slice(bytes) {
            Ok(key) => Ok(key),
            Err(rustls::pki_types::pem::Error::NoItemsFound) => {
                Err(TlsConfigError::NoPrivateKey("<inline bytes>".to_owned()))
            }
            Err(source) => Err(TlsConfigError::PemBytes(source)),
        };
    }
    match &spec.key_store_path {
        Some(path) => load_private_key(path),
        None => Err(TlsConfigError::MissingKeyStore),
    }
}

/// T166/T167 (feature 009, NEW-95): count `RootCertStore::add` failures (a malformed cert,
/// unsupported key type, etc. — each individually best-effort-skipped, matching the pre-existing
/// per-cert tolerance) and warn when any occurred, so a partially-broken trust store is at least
/// visible instead of silently thinner than configured.
fn add_certs_tracking_failures(roots: &mut RootCertStore, certs: Vec<CertificateDer<'static>>) {
    let mut failed = 0usize;
    for cert in certs {
        if roots.add(cert).is_err() {
            failed += 1;
        }
    }
    if failed > 0 {
        tracing::warn!(
            failed,
            "trust store: {failed} certificate(s) failed to load"
        );
    }
}

fn load_root_store(path: &Path) -> Result<RootCertStore, TlsConfigError> {
    let mut roots = RootCertStore::empty();
    add_certs_tracking_failures(&mut roots, load_certs(path)?);
    if roots.is_empty() {
        return Err(TlsConfigError::EmptyTrustStore(path.display().to_string()));
    }
    Ok(roots)
}

fn load_root_store_bytes(bytes: &[u8]) -> Result<RootCertStore, TlsConfigError> {
    let mut roots = RootCertStore::empty();
    let certs = CertificateDer::pem_slice_iter(bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(TlsConfigError::PemBytes)?;
    add_certs_tracking_failures(&mut roots, certs);
    if roots.is_empty() {
        return Err(TlsConfigError::EmptyTrustStore("<inline bytes>".to_owned()));
    }
    Ok(roots)
}

/// The trust store, from inline bytes when set (`SocketTrustStoreBytes`), otherwise from
/// `SocketTrustStore`'s file path; `None` when neither is configured.
fn trust_store(spec: &TlsSpec) -> Result<Option<RootCertStore>, TlsConfigError> {
    if let Some(bytes) = &spec.trust_store_bytes {
        return Ok(Some(load_root_store_bytes(bytes)?));
    }
    match &spec.trust_store_path {
        Some(path) => Ok(Some(load_root_store(path)?)),
        None => Ok(None),
    }
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

/// The process-default `CryptoProvider`, optionally filtered down to `names` (`CipherSuites`;
/// FR-017) — each entry matched case-insensitively against the suite's `Debug` name (e.g.
/// `"TLS13_AES_128_GCM_SHA256"`). An empty `names` list preserves rustls's default suite set
/// unchanged.
fn crypto_provider(names: &[String]) -> Result<Arc<CryptoProvider>, TlsConfigError> {
    let base = rustls::crypto::aws_lc_rs::default_provider();
    if names.is_empty() {
        return Ok(Arc::new(base));
    }
    let unrecognized: Vec<_> = names
        .iter()
        .filter(|name| {
            !base.cipher_suites.iter().any(|suite| {
                let suite_name = format!("{:?}", suite.suite());
                name.eq_ignore_ascii_case(&suite_name)
            })
        })
        .cloned()
        .collect();
    if !unrecognized.is_empty() {
        return Err(TlsConfigError::UnrecognizedCipherSuites(unrecognized));
    }

    let cipher_suites: Vec<_> = base
        .cipher_suites
        .iter()
        .filter(|suite| {
            let suite_name = format!("{:?}", suite.suite());
            names.iter().any(|n| n.eq_ignore_ascii_case(&suite_name))
        })
        .copied()
        .collect();
    Ok(Arc::new(CryptoProvider {
        cipher_suites,
        ..base
    }))
}

/// Build a server-side (acceptor) TLS configuration from `spec` (FR-017), including mTLS when
/// `need_client_auth` is set.
pub fn build_server_config(spec: &TlsSpec) -> Result<Arc<ServerConfig>, TlsConfigError> {
    let certs = key_store_certs(spec)?;
    if certs.is_empty() {
        return Err(TlsConfigError::NoCertificate(key_store_label(spec)));
    }
    let key = key_store_private_key(spec)?;
    let provider = crypto_provider(&spec.cipher_suites)?;
    let builder = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(protocol_versions(spec.min_version))?;

    let config = if spec.need_client_auth {
        let roots =
            Arc::new(trust_store(spec)?.ok_or(TlsConfigError::MissingTrustStoreForClientAuth)?);
        let verifier = WebPkiClientVerifier::builder(roots).build()?;
        builder
            .with_client_cert_verifier(verifier)
            .with_single_cert(certs, key)?
    } else {
        builder.with_no_client_auth().with_single_cert(certs, key)?
    };
    Ok(Arc::new(config))
}

/// NEW-11 (feature 009): the OS-native trust store, used as `build_client_config`'s fallback when
/// no explicit `SocketTrustStore`/`SocketTrustStoreBytes` is configured — matches QuickFIX/J's own
/// fallback to the JVM's default (OS-integrated) trust store, rather than an empty `RootCertStore`
/// that rejects every server certificate. Per-certificate load failures are silently skipped here
/// (consistent with `load_root_store`'s existing best-effort style; NEW-95 tracks adding load
/// diagnostics to both).
fn native_root_store() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    roots
}

/// Build a client-side (initiator) TLS configuration from `spec` (FR-017). When
/// `need_client_auth` is set, the initiator also presents the key store as its own client
/// certificate (mTLS).
pub fn build_client_config(spec: &TlsSpec) -> Result<Arc<ClientConfig>, TlsConfigError> {
    let roots = match trust_store(spec)? {
        Some(store) => store,
        None => native_root_store(),
    };
    let provider = crypto_provider(&spec.cipher_suites)?;
    let builder = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(protocol_versions(spec.min_version))?
        .with_root_certificates(roots);

    let config = if spec.need_client_auth {
        let certs = key_store_certs(spec)?;
        let key = key_store_private_key(spec)?;
        builder.with_client_auth_cert(certs, key)?
    } else {
        builder.with_no_client_auth()
    };
    Ok(Arc::new(config))
}

fn key_store_label(spec: &TlsSpec) -> String {
    if spec.key_store_bytes.is_some() {
        "<inline bytes>".to_owned()
    } else {
        spec.key_store_path
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod native_trust_store_tests {
    use super::*;

    // T026/T027 (US1, feature 009, `NEW-11`): `build_client_config` fell back to an empty
    // `RootCertStore` when no explicit `SocketTrustStore`/`SocketTrustStoreBytes` was configured,
    // making TLS unusable without one -- unlike QuickFIX/J, which falls back to the JVM's default
    // (OS-integrated) trust store. A full end-to-end handshake test against a real
    // publicly-trusted-CA-signed certificate isn't possible in this environment (no outbound
    // network access, and a locally-minted test CA is deliberately absent from the OS trust
    // store, so it can't stand in for one) -- this test instead verifies the core mechanism the
    // fix relies on directly: `native_root_store()` actually loads certificates from this
    // machine's OS-native trust store.
    #[test]
    fn native_root_store_loads_at_least_one_certificate() {
        let roots = native_root_store();
        assert!(
            !roots.is_empty(),
            "native_root_store() must load at least one certificate from the OS trust store on \
             any normally-configured machine -- an empty result means the NEW-11 fallback is not \
             actually providing usable trust anchors"
        );
    }

    #[test]
    fn build_client_config_with_no_explicit_trust_store_still_builds_successfully() {
        // `rustls::ClientConfig` doesn't expose a public way to introspect its root store's
        // contents, so this only confirms `build_client_config` still succeeds when falling back
        // to the native store (it also "succeeded" before this fix, producing an unusable empty
        // store -- `native_root_store_loads_at_least_one_certificate` above is the test that
        // actually proves the fallback is non-empty).
        let spec = TlsSpec {
            key_store_path: None,
            trust_store_path: None,
            key_store_bytes: None,
            trust_store_bytes: None,
            need_client_auth: false,
            min_version: None,
            server_name: None,
            cipher_suites: Vec::new(),
        };
        build_client_config(&spec).expect("client config should build using the native fallback");
    }
}
