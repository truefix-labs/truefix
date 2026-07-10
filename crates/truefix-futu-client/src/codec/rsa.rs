use std::fs;
use std::path::Path;

use rand::rngs::OsRng;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::Pkcs1v15Encrypt;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};

use crate::error::{FutuError, FutuResult};

#[derive(Debug, Clone)]
pub struct InitRsaCipher {
    private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
}

impl InitRsaCipher {
    pub fn from_pkcs1_pem_file(path: impl AsRef<Path>) -> FutuResult<Self> {
        let pem = fs::read_to_string(path).map_err(FutuError::Io)?;
        let private_key = RsaPrivateKey::from_pkcs1_pem(&pem)
            .map_err(|err| FutuError::Crypto(format!("invalid RSA private key: {err}")))?;
        let public_key = RsaPublicKey::from(&private_key);
        Ok(Self {
            private_key,
            public_key,
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> FutuResult<Vec<u8>> {
        let mut rng = OsRng;
        let max_chunk = self
            .public_key
            .size()
            .checked_sub(11)
            .ok_or_else(|| FutuError::Crypto("RSA key size too small".to_owned()))?;
        let mut out = Vec::new();
        for chunk in plaintext.chunks(max_chunk) {
            let encrypted = self
                .public_key
                .encrypt(&mut rng, Pkcs1v15Encrypt, chunk)
                .map_err(|err| FutuError::Crypto(format!("RSA encrypt failed: {err}")))?;
            out.extend_from_slice(&encrypted);
        }
        Ok(out)
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> FutuResult<Vec<u8>> {
        let block_size = self.private_key.size();
        if block_size == 0 || !ciphertext.len().is_multiple_of(block_size) {
            return Err(FutuError::Crypto(
                "invalid RSA ciphertext length".to_owned(),
            ));
        }
        let mut out = Vec::new();
        for chunk in ciphertext.chunks(block_size) {
            let decrypted = self
                .private_key
                .decrypt(Pkcs1v15Encrypt, chunk)
                .map_err(|err| FutuError::Crypto(format!("RSA decrypt failed: {err}")))?;
            out.extend_from_slice(&decrypted);
        }
        Ok(out)
    }
}
