//! Middleware d'authentification JWT et gestion des refresh tokens.
//!
//! Fournit les fonctions de création/vérification de tokens JWT
//! et la gestion sécurisée des refresh tokens avec rotation.

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use mongodb::Database;
use bson::doc;
use uuid::Uuid;

use crate::errors::AppError;

/// Claims contenus dans le token JWT
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,        // user_id
    pub username: String,
    pub exp: usize,      // expiration timestamp
    pub iat: usize,      // issued at timestamp
}

/// Crée un token JWT signé avec la clé secrète (durée de vie : 24h)
pub fn create_token(user_id: i32, username: &str, jwt_secret: &str) -> Result<String, AppError> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::hours(24)).timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Auth(format!("Erreur création token: {e}")))
}

/// Vérifie et décode un token JWT
pub fn verify_token(token: &str, jwt_secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Auth(format!("Token invalide: {e}")))
}

// ───────── Refresh Tokens ─────────

/// Hash le refresh token (SHA-256) avant stockage en base
fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Génère un refresh token, le stocke en base (hashé), et retourne le token brut.
/// Durée de vie : 7 jours. Le token est à usage unique (rotation).
pub async fn create_refresh_token(db: &Database, user_id: i32) -> Result<String, AppError> {
    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::days(7);
    let new_id = crate::db::next_id(db, "refresh_tokens").await?;

    db.collection::<bson::Document>("refresh_tokens")
        .insert_one(
            doc! {
                "_id": new_id,
                "user_id": user_id,
                "token_hash": &token_hash,
                "expires_at": bson::DateTime::from_chrono(expires_at),
                "created_at": bson::DateTime::from_chrono(Utc::now()),
            },
            None,
        )
        .await?;

    Ok(raw_token)
}

/// Valide et consomme un refresh token (rotation : supprimé après usage).
/// Retourne le user_id si le token est valide et non expiré.
pub async fn verify_and_rotate_refresh_token(
    db: &Database,
    raw_token: &str,
) -> Result<i32, AppError> {
    let token_hash = hash_token(raw_token);

    #[derive(serde::Deserialize)]
    struct RefreshToken {
        user_id: i32,
        #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
        expires_at: chrono::DateTime<Utc>,
    }

    // Suppression atomique : find_one_and_delete empêche la réutilisation
    let result = db
        .collection::<RefreshToken>("refresh_tokens")
        .find_one_and_delete(doc! { "token_hash": &token_hash }, None)
        .await?;

    match result {
        Some(rt) => {
            if rt.expires_at < Utc::now() {
                Err(AppError::Auth("Refresh token expiré".to_string()))
            } else {
                Ok(rt.user_id)
            }
        }
        None => Err(AppError::Auth("Refresh token invalide".to_string())),
    }
}

/// Supprime tous les refresh tokens d'un utilisateur (logout global)
pub async fn delete_all_refresh_tokens(db: &Database, user_id: i32) -> Result<(), AppError> {
    db.collection::<bson::Document>("refresh_tokens")
        .delete_many(doc! { "user_id": user_id }, None)
        .await?;
    Ok(())
}
