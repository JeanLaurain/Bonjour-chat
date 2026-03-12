//! Middleware et utilitaires d'authentification JWT.
//!
//! Gère la création et la vérification des tokens JWT.
//! Les tokens ont une durée de vie de 24 heures et contiennent
//! l'ID utilisateur et le nom d'utilisateur dans les claims.

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::config::AppState;
use crate::errors::AppError;

/// Claims JWT embarqués dans chaque token.
/// - `sub` : identifiant unique de l'utilisateur
/// - `username` : nom d'utilisateur (pour éviter un appel DB supplémentaire)
/// - `exp` : timestamp d'expiration (Unix epoch)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,
    pub username: String,
    pub exp: usize,
}

/// Crée un token JWT signé avec HS256, valide 24 heures.
/// Retourne le token encodé sous forme de String.
pub fn create_token(user_id: i32, username: &str, secret: &str) -> Result<String, AppError> {
    // Le token expire dans 24h à partir de maintenant
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        exp: expiration,
    };

    // Encodage avec l'algorithme par défaut (HS256)
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Vérifie et décode un token JWT.
/// Retourne les claims si le token est valide et non expiré.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

/// Middleware Axum générique pour protéger des routes.
/// Extrait le header `Authorization: Bearer <token>`, vérifie le JWT,
/// puis injecte les Claims dans les extensions de la requête pour
/// que les handlers en aval puissent y accéder.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Extraction du header Authorization
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    // Le format attendu est "Bearer <token>"
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    // Vérification et décodage du token
    let claims = verify_token(token, &state.jwt_secret)?;

    // Injection des claims dans les extensions de la requête
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
