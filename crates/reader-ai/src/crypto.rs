use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM},
    rand::{SecureRandom, SystemRandom},
};
use uuid::Uuid;

use crate::AiError;

/// AES-256-GCM credential encryption. Random 96-bit nonce per write; account UUID
/// is authenticated data, so copying ciphertext between accounts cannot decrypt.
/// The 32-byte master key lives outside PostgreSQL and must be backed up separately.
pub struct CredentialCipher(LessSafeKey);

impl CredentialCipher {
    pub fn new(master: &[u8]) -> Result<Self, AiError> {
        let key = UnboundKey::new(&AES_256_GCM, master).map_err(|_| AiError::Encryption)?;
        Ok(Self(LessSafeKey::new(key)))
    }
    pub fn encrypt(&self, account: Uuid, plaintext: &str) -> Result<Vec<u8>, AiError> {
        let mut nonce = [0; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| AiError::Encryption)?;
        let mut body = plaintext.as_bytes().to_vec();
        self.0
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(account.as_bytes()),
                &mut body,
            )
            .map_err(|_| AiError::Encryption)?;
        let mut encrypted = nonce.to_vec();
        encrypted.extend(body);
        Ok(encrypted)
    }
    pub fn decrypt(&self, account: Uuid, ciphertext: &[u8]) -> Result<String, AiError> {
        let (nonce, body) = ciphertext.split_at_checked(12).ok_or(AiError::Encryption)?;
        let mut body = body.to_vec();
        let plaintext = self
            .0
            .open_in_place(
                Nonce::try_assume_unique_for_key(nonce).map_err(|_| AiError::Encryption)?,
                Aad::from(account.as_bytes()),
                &mut body,
            )
            .map_err(|_| AiError::Encryption)?;
        String::from_utf8(plaintext.to_vec()).map_err(|_| AiError::Encryption)
    }
}
