//! Gestion centralisée des erreurs de l'application.
//!
//! Toutes les erreurs sont converties en réponses HTTP JSON grâce à
//! l'implémentation de `IntoResponse` pour `AppError`.
//! Les types de réponse (AuthResponse, SendMessageResponse, etc.) sont
//! définis ici pour être réutilisés dans les annotations Swagger (utoipa).

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
    /// Description de l'erreur
    pub error: String,
}

/// Réponse renvoyée après inscription ou connexion réussie
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    /// Message de confirmation
    pub message: String,
    /// Informations de l'utilisateur
    pub user: UserInfo,
    /// Token JWT à utiliser dans le header Authorization
    pub token: String,
}

/// Informations basiques d'un utilisateur (sans mot de passe)
#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: u64,
    pub username: String,
    pub email: String,
}

/// Réponse renvoyée après l'envoi d'un message
#[derive(Debug, Serialize, ToSchema)]
pub struct SendMessageResponse {
    /// Message de confirmation
    pub message: String,
    /// ID du message créé
    pub id: u64,
    /// ID de l'expéditeur
    pub sender_id: i32,
    /// ID du destinataire
    pub receiver_id: i32,
}

/// Réponse contenant les messages d'une conversation
#[derive(Debug, Serialize, ToSchema)]
pub struct ConversationResponse {
    /// ID de l'interlocuteur
    pub conversation_with: i32,
    /// Liste des messages échangés, triés chronologiquement
    pub messages: Vec<crate::models::message::Message>,
}

/// Réponse contenant la liste de toutes les conversations actives
#[derive(Debug, Serialize, ToSchema)]
pub struct ConversationsListResponse {
    /// Aperçu de chaque conversation (dernier message)
    pub conversations: Vec<crate::models::message::ConversationPreview>,
}

/// Réponse contenant les résultats de recherche d'utilisateurs
#[derive(Debug, Serialize, ToSchema)]
pub struct UsersSearchResponse {
    /// Liste des utilisateurs correspondant à la recherche
    pub users: Vec<crate::models::user::UserResponse>,
}

// ──────────────────────────────────────────────
//  Enum d'erreurs applicatives
// ──────────────────────────────────────────────

/// Erreurs possibles dans l'application.
/// Chaque variante est mappée vers un code HTTP et un message JSON.
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

    /// Erreur de base de données (sqlx) — loguée côté serveur,
    /// jamais exposée au client
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Erreur de création/vérification JWT
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    /// Erreur de hachage bcrypt
    #[error("Bcrypt error: {0}")]
    Bcrypt(#[from] bcrypt::BcryptError),
}

/// Conversion automatique de `AppError` en réponse HTTP.
/// Les erreurs internes (DB, bcrypt) sont masquées au client
/// et loguées côté serveur pour le debug.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::InvalidCredentials => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::UserAlreadyExists => (StatusCode::CONFLICT, self.to_string()),
            AppError::UserNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),

            // Erreurs internes : on log le détail mais on renvoie un message générique
            AppError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
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

        // Toutes les erreurs sont renvoyées au format { "error": "..." }
        (status, Json(json!({ "error": message }))).into_response()
    }
}
