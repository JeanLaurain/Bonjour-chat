//! Module de chiffrement AES-256-GCM pour les données sensibles.
//!
//! Chiffre les noms d'utilisateur, emails et contenus de messages
//! avec AES-256-GCM. Les données chiffrées sont préfixées par "ENC:"
//! pour permettre la coexistence avec des données en clair (migration).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Préfixe identifiant une valeur chiffrée
const ENC_PREFIX: &str = "ENC:";

/// Chiffre une chaîne avec AES-256-GCM.
/// Le résultat est au format `ENC:<base64(nonce_12_octets + ciphertext + tag)>`.
pub fn encrypt(plaintext: &str, key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new(key.into());

    // Générer un nonce aléatoire de 12 octets
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Chiffrer le texte
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Erreur de chiffrement: {}", e))?;

    // Concaténer nonce + ciphertext et encoder en base64
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(format!("{}{}", ENC_PREFIX, BASE64.encode(&combined)))
}

/// Déchiffre une chaîne au format `ENC:<base64(...)>`.
/// Si la chaîne n'est pas chiffrée (pas de préfixe "ENC:"),
/// elle est retournée telle quelle (compatibilité données existantes).
pub fn try_decrypt(value: &str, key: &[u8; 32]) -> String {
    // Si ce n'est pas chiffré, retourner tel quel
    if !value.starts_with(ENC_PREFIX) {
        return value.to_string();
    }

    let encoded = &value[ENC_PREFIX.len()..];

    // Décoder le base64
    let combined = match BASE64.decode(encoded) {
        Ok(data) => data,
        Err(_) => return value.to_string(),
    };

    // Vérifier la taille minimale (12 octets nonce + au moins 16 octets tag)
    if combined.len() < 28 {
        return value.to_string();
    }

    // Séparer le nonce et le ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new(key.into());

    // Déchiffrer
    match cipher.decrypt(nonce, ciphertext) {
        Ok(plaintext) => String::from_utf8(plaintext).unwrap_or_else(|_| value.to_string()),
        Err(_) => value.to_string(),
    }
}

/// Calcule le hash SHA-256 d'une chaîne (pour les lookups en base).
/// Retourne une chaîne hexadécimale de 64 caractères.
pub fn hash_value(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = i as u8;
        }
        key
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = test_key();
        let original = "Bonjour le monde!";
        let encrypted = encrypt(original, &key).unwrap();
        assert!(encrypted.starts_with(ENC_PREFIX));
        let decrypted = try_decrypt(&encrypted, &key);
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_try_decrypt_plaintext() {
        let key = test_key();
        let plaintext = "texte en clair";
        let result = try_decrypt(plaintext, &key);
        assert_eq!(result, plaintext);
    }

    #[test]
    fn test_hash_deterministic() {
        let h1 = hash_value("alice");
        let h2 = hash_value("alice");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }
}
