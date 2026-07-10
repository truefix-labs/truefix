use aes::Aes128;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit, KeyIvInit, generic_array::GenericArray};
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, block_padding::Pkcs7};
use cbc::{Decryptor, Encryptor};

use crate::error::{FutuError, FutuResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncAlgo {
    None,
    FtAesEcb { key: [u8; 16] },
    AesCbc { key: [u8; 16], iv: [u8; 16] },
}

impl EncAlgo {
    pub fn encrypt(&self, plaintext: &[u8]) -> FutuResult<Vec<u8>> {
        match self {
            Self::None => Ok(plaintext.to_vec()),
            Self::FtAesEcb { key } => encrypt_ft_aes_ecb(*key, plaintext),
            Self::AesCbc { key, iv } => {
                let cipher = Encryptor::<Aes128>::new(key.into(), iv.into());
                let mut buf = plaintext.to_vec();
                let orig_len = buf.len();
                buf.resize(orig_len + 16, 0);
                let out = cipher
                    .encrypt_padded_mut::<Pkcs7>(&mut buf, orig_len)
                    .map_err(|err| FutuError::Crypto(err.to_string()))?;
                Ok(out.to_vec())
            }
        }
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> FutuResult<Vec<u8>> {
        match self {
            Self::None => Ok(ciphertext.to_vec()),
            Self::FtAesEcb { key } => decrypt_ft_aes_ecb(*key, ciphertext),
            Self::AesCbc { key, iv } => {
                if !ciphertext.len().is_multiple_of(16) {
                    return Err(FutuError::Crypto(
                        "ciphertext length must be multiple of 16".into(),
                    ));
                }
                let cipher = Decryptor::<Aes128>::new(key.into(), iv.into());
                let mut buf = ciphertext.to_vec();
                let out = cipher
                    .decrypt_padded_mut::<Pkcs7>(&mut buf)
                    .map_err(|err| FutuError::Crypto(err.to_string()))?;
                Ok(out.to_vec())
            }
        }
    }
}

fn encrypt_ft_aes_ecb(key: [u8; 16], plaintext: &[u8]) -> FutuResult<Vec<u8>> {
    let body_len = plaintext.len().div_ceil(16) * 16;
    let mut out = vec![0u8; body_len + 16];
    let body = out
        .get_mut(..body_len)
        .ok_or_else(|| FutuError::Crypto("ftaes body length overflow".into()))?;
    body.get_mut(..plaintext.len())
        .ok_or_else(|| FutuError::Crypto("ftaes body length overflow".into()))?
        .copy_from_slice(plaintext);
    let (body, tail) = out.split_at_mut(body_len);
    let cipher = Aes128::new(&key.into());
    for chunk in body.chunks_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        cipher.encrypt_block(block);
    }
    tail.get_mut(..8)
        .ok_or_else(|| FutuError::Crypto("ftaes tail length overflow".into()))?
        .copy_from_slice(&(plaintext.len() as u64).to_le_bytes());
    Ok(out)
}

fn decrypt_ft_aes_ecb(key: [u8; 16], ciphertext: &[u8]) -> FutuResult<Vec<u8>> {
    if ciphertext.len() < 16 || !ciphertext.len().is_multiple_of(16) {
        return Err(FutuError::Crypto(
            "ciphertext length must be multiple of 16".into(),
        ));
    }
    let body_len = ciphertext.len() - 16;
    let mut out = ciphertext
        .get(..body_len)
        .ok_or_else(|| FutuError::Crypto("invalid FTAES ciphertext length".into()))?
        .to_vec();
    let cipher = Aes128::new(&key.into());
    for chunk in out.chunks_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }
    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(
        ciphertext
            .get(body_len..body_len + 8)
            .ok_or_else(|| FutuError::Crypto("invalid FTAES tail length".into()))?,
    );
    let plain_len = usize::try_from(u64::from_le_bytes(len_bytes))
        .map_err(|_| FutuError::Crypto("plaintext length overflow".into()))?;
    if plain_len > out.len() {
        return Err(FutuError::Crypto("invalid FTAES tail length".into()));
    }
    out.truncate(plain_len);
    Ok(out)
}
