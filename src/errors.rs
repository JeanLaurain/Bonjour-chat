//! Gestion centralisée des erreurs de l'application.
//!
//! Toutes les erreurs sont converties en réponses HTTP JSON grâce à
//! l'implémentation de `IntoResponse` pour `AppError`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

// ──────────────────────────────────────────────
//  Schémas de réponse pour Swagger
// ──────────────────────────────────────────────

/// Réponse d'erreur standard renvoyée par tous les endpoints
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

/// Réponse renvoyée après inscription ou connexion réussie
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub message: String,
    pub user: UserInfo,
    pub token: String,
}

/// Informations basiques d'un utilisateur (sans mot de passe)
#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: i32,
    pub username: String,
    pub email: String,
}

/// Réponse renvoyée après l'envoi d'un message
#[derive(Debug, Serialize, ToSchema)]
pub struct SendMessageResponse {
    pub message: String,
    pub id: i32,
    pub sender_id: i32,
    pub receiver_id: i32,
}

/// Réponse contenant les messages d'une conversation
#[derive(Debug, Serialize, ToSchema)]
pub struct ConversationResponse {
    pub conversation_with: i32,
    pub messages: Vec<crate::models::message::Message>,
}

/// Réponse contenant la liste de toutes les conversations actives
#[derive(Debug, Serialize, ToSchema)]
pub struct ConversationsListResponse {
    pub conversations: Vec<crate::models::message::ConversationPreview>,
}

/// Réponse contenant les résultats de recherche d'utilisateurs
#[derive(Debug, Serialize, ToSchema)]
pub struct UsersSearchResponse {
    pub users: Vec<crate::models::user::UserResponse>,
}

// ──────────────────────────────────────────────
//  Enum d'erreurs applicatives
// ──────────────────────────────────────────────

/// Erreurs possibles dans l'application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("User already exists")]
    UserAlreadyExists,

    #[error("User not found")]
    UserNotFound,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Validation error: {0}")]
    Validation(String),

    /// Erreur de base de données MongoDB
    #[error("Database error: {0}")]
    Database(String),

    /// Erreur d'authentification (token invalide, expiré, etc.)
    #[error("Auth error: {0}")]
    Auth(String),

    /// Erreur de création/vérification JWT
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    /// Erreur de hachage bcrypt
    #[error("Bcrypt error: {0}")]
    Bcrypt(#[from] bcrypt::BcryptError),
}

/// Conversion depuis les erreurs MongoDB
impl From<mongodb::error::Error> for AppError {
    fn from(e: mongodb::error::Error) -> Self {
        AppError::Database(e.to_string())
    }
}

/// Conversion depuis les erreurs de désérialisation BSON
impl From<bson::de::Error> for AppError {
    fn from(e: bson::de::Error) -> Self {
        AppError::Database(format!("BSON error: {}", e))
    }
}

/// Conversion depuis les erreurs de sérialisation BSON
impl From<bson::ser::Error> for AppError {
    fn from(e: bson::ser::Error) -> Self {
        AppError::Database(format!("BSON error: {}", e))
    }
}

/// Conversion automatique de `AppError` en réponse HTTP.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::InvalidCredentials => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::UserAlreadyExists => (StatusCode::CONFLICT, self.to_string()),
            AppError::UserNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Auth(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            AppError::Jwt(e) => {
                tracing::error!("JWT error: {:?}", e);
                (StatusCode::UNAUTHORIZED, "Invalid token".to_string())
            }
            AppError::Bcrypt(e) => {
                tracing::error!("Bcrypt error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
