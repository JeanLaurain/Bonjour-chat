//! Handler de recherche d'utilisateurs.
//!
//! Permet aux utilisateurs authentifiés de rechercher d'autres utilisateurs
//! par nom d'utilisateur pour démarrer de nouvelles conversations.

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::AppState;
use crate::errors::AppError;
use crate::middleware::auth::verify_token;
use crate::models::user;

/// Paramètres de requête pour la recherche d'utilisateurs
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Terme de recherche (minimum 2 caractères)
    pub q: String,
}

/// Recherche d'utilisateurs par nom d'utilisateur.
///
/// Nécessite un JWT valide. Retourne les utilisateurs dont le nom
/// correspond partiellement au terme de recherche, en excluant
/// l'utilisateur connecté.
#[utoipa::path(
    get,
    path = "/users/search",
    tag = "Users",
    params(("q" = String, Query, description = "Terme de recherche (min 2 caractères)")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Liste des utilisateurs trouvés", body = UsersSearchResponse),
        (status = 401, description = "Non autorisé", body = crate::errors::ErrorResponse),
    )
)]
pub async fn search_users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Value>, AppError> {
    // Extraction et vérification du token JWT depuis les headers
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    let claims = verify_token(token, &state.jwt_secret)?;

    // Recherche minimale de 2 caractères
    if params.q.len() < 2 {
        return Ok(Json(json!({ "users": [] })));
    }

    // Recherche en base de données (déchiffre puis filtre, exclut l'utilisateur connecté)
    let users = user::search_users(&state.db, &params.q, claims.sub, &state.encryption_key).await?;

    Ok(Json(json!({ "users": users })))
}
