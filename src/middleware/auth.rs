//! Middleware et utilitaires d'authentification JWT.
//!
//! Gère la création et la vérification des tokens JWT (access + refresh).
//! - Access token : durée de vie de 1 heure (usage courant des requêtes API).
//! - Refresh token : durée de vie de 7 jours (permet de renouveler l'access token).

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

use crate::config::AppState;
use crate::errors::AppError;

/// Claims JWT embarqués dans chaque access token.
/// - `sub` : identifiant unique de l'utilisateur
/// - `username` : nom d'utilisateur (pour éviter un appel DB supplémentaire)
/// - `exp` : timestamp d'expiration (Unix epoch)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,
    pub username: String,
    pub exp: usize,
}

/// Crée un access token JWT signé avec HS256, valide 1 heure.
pub fn create_token(user_id: i32, username: &str, secret: &str) -> Result<String, AppError> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(1))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Vérifie et décode un access token JWT.
/// Retourne les claims si le token est valide et non expiré.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

/// Génère un refresh token aléatoire (UUID v4) et le stocke hashé en base.
/// Retourne le token brut (à envoyer au client) — seul le hash est persisté.
pub async fn create_refresh_token(
    db: &sqlx::MySqlPool,
    user_id: i32,
) -> Result<String, AppError> {
    let raw_token = uuid::Uuid::new_v4().to_string();
    let token_hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);

    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at.naive_utc())
        .execute(db)
        .await?;

    Ok(raw_token)
}

/// Valide un refresh token : vérifie qu'il existe, n'est pas expiré,
/// puis le supprime (rotation — un refresh token n'est utilisable qu'une fois).
/// Retourne le user_id associé si tout est OK.
pub async fn verify_and_rotate_refresh_token(
    db: &sqlx::MySqlPool,
    raw_token: &str,
) -> Result<i32, AppError> {
    let token_hash = hex::encode(Sha256::digest(raw_token.as_bytes()));

    // Récupérer le refresh token correspondant
    let row: Option<(i32, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT user_id, expires_at FROM refresh_tokens WHERE token_hash = ?"
    )
        .bind(&token_hash)
        .fetch_optional(db)
        .await?;

    let (user_id, expires_at) = row.ok_or(AppError::Unauthorized)?;

    // Vérifier l'expiration
    if expires_at < chrono::Utc::now().naive_utc() {
        // Supprimer le token expiré
        sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = ?")
            .bind(&token_hash)
            .execute(db)
            .await?;
        return Err(AppError::Unauthorized);
    }

    // Supprimer l'ancien token (rotation : chaque refresh token est à usage unique)
    sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = ?")
        .bind(&token_hash)
        .execute(db)
        .await?;

    Ok(user_id)
}

/// Supprime tous les refresh tokens d'un utilisateur (logout global).
pub async fn delete_all_refresh_tokens(
    db: &sqlx::MySqlPool,
    user_id: i32,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = ?")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Nettoie les refresh tokens expirés (maintenance).
pub async fn cleanup_expired_tokens(db: &sqlx::MySqlPool) -> Result<(), AppError> {
    sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < NOW()")
        .execute(db)
        .await?;
    Ok(())
}

/// Middleware Axum générique pour protéger des routes.
/// Extrait le header `Authorization: Bearer <token>`, vérifie le JWT,
/// puis injecte les Claims dans les extensions de la requête.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    let claims = verify_token(token, &state.jwt_secret)?;

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
