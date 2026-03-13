//! Handlers de messagerie (envoi, conversation, liste).
//!
//! Tous ces endpoints nécessitent un JWT valide dans le header
//! `Authorization: Bearer <token>`. L'identité de l'utilisateur
//! est extraite des claims JWT pour chaque requête.

use axum::{
    extract::{Path, Query, Request, State},
    http::header,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::AppState;
use crate::errors::{AppError, ConversationResponse, ConversationsListResponse, ErrorResponse, SendMessageResponse};
use crate::middleware::auth::{verify_token, Claims};
use crate::models::{message, user};

/// Paramètres de pagination pour les messages
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    /// ID du message avant lequel charger (pour scroll infini)
    pub before_id: Option<i32>,
    /// Nombre de messages à charger (défaut: 10)
    pub limit: Option<i64>,
}

/// Extrait les claims JWT depuis le header Authorization de la requête.
/// Retourne une erreur `Unauthorized` si le header est absent ou le token invalide.
fn extract_claims(req: &Request, jwt_secret: &str) -> Result<Claims, AppError> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    // Le format attendu est "Bearer <token>"
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    verify_token(token, jwt_secret)
}

/// Envoie un message direct à un autre utilisateur.
///
/// Vérifie que :
/// - Le token JWT est valide
/// - Le contenu du message fait entre 1 et 5000 caractères
/// - Le destinataire existe en base
/// - L'expéditeur n'essaie pas de s'envoyer un message à lui-même
#[utoipa::path(
    post,
    path = "/messages",
    tag = "Messages",
    request_body = message::CreateMessage,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Message envoyé", body = SendMessageResponse),
        (status = 400, description = "Erreur de validation", body = ErrorResponse),
        (status = 401, description = "Non autorisé", body = ErrorResponse),
        (status = 404, description = "Destinataire introuvable", body = ErrorResponse),
    )
)]
pub async fn send_message(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    // Extraction de l'identité de l'expéditeur depuis le JWT
    let claims = extract_claims(&req, &state.jwt_secret)?;

    // Lecture du corps de la requête (limité à 1 Mo)
    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

    // Désérialisation du JSON
    let payload: message::CreateMessage = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON body".to_string()))?;

    // Validation de la longueur du contenu
    if payload.message_type == "text" && (payload.content.is_empty() || payload.content.len() > 5000) {
        return Err(AppError::Validation(
            "Message content must be between 1 and 5000 characters".to_string(),
        ));
    }

    // Pour les images, l'URL doit être fournie
    if payload.message_type == "image" && payload.image_url.is_none() {
        return Err(AppError::Validation(
            "image_url is required for image messages".to_string(),
        ));
    }

    // Vérification que le destinataire existe en base
    user::find_by_id(&state.db, payload.receiver_id, &state.encryption_key)
        .await?
        .ok_or(AppError::UserNotFound)?;

    // Interdiction de s'envoyer un message à soi-même
    if payload.receiver_id == claims.sub {
        return Err(AppError::Validation(
            "Cannot send a message to yourself".to_string(),
        ));
    }

    // Insertion du message en base (contenu chiffré)
    let message_id = message::create_message(
        &state.db,
        claims.sub,
        payload.receiver_id,
        &payload.content,
        &payload.message_type,
        payload.image_url.as_deref(),
        &state.encryption_key,
    )
    .await?;

    // Notification WebSocket temps réel au destinataire et à l'expéditeur
    let ws_msg = serde_json::json!({
        "type": "new_message",
        "data": {
            "id": message_id,
            "sender_id": claims.sub,
            "receiver_id": payload.receiver_id,
            "content": payload.content,
            "message_type": payload.message_type,
            "image_url": payload.image_url,
            "created_at": chrono::Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string()
        }
    })
    .to_string();

    crate::handlers::ws::send_to_user(&state, payload.receiver_id, &ws_msg).await;
    crate::handlers::ws::send_to_user(&state, claims.sub, &ws_msg).await;

    // Si le destinataire n'a pas de connexion WebSocket active, envoyer une notification push
    let has_ws = {
        let conns = state.ws_connections.read().await;
        conns.get(&payload.receiver_id).map(|s| !s.is_empty()).unwrap_or(false)
    };
    if !has_ws {
        // Récupérer le nom de l'expéditeur pour la notification
        let sender_name = user::find_by_id(&state.db, claims.sub, &state.encryption_key)
            .await
            .ok()
            .flatten()
            .map(|u| u.username)
            .unwrap_or_else(|| "Quelqu'un".to_string());

        let state_clone = state.clone();
        tokio::spawn(async move {
            crate::handlers::notifications::send_push_to_user(&state_clone, payload.receiver_id, &sender_name).await;
        });
    }

    Ok(Json(json!({
        "message": "Message sent successfully",
        "id": message_id,
        "sender_id": claims.sub,
        "receiver_id": payload.receiver_id
    })))
}

/// Récupère les messages échangés avec un utilisateur (paginé).
///
/// Par défaut, retourne les 10 derniers messages.
/// Utiliser `?before_id=X` pour charger les messages plus anciens (scroll infini).
/// Utiliser `?limit=N` pour contrôler le nombre de messages retournés.
#[utoipa::path(
    get,
    path = "/conversations/{user_id}",
    tag = "Messages",
    params(
        ("user_id" = i32, Path, description = "ID de l'interlocuteur"),
        ("before_id" = Option<i32>, Query, description = "Charger les messages avant cet ID"),
        ("limit" = Option<i64>, Query, description = "Nombre de messages (défaut: 10)")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Messages de la conversation", body = ConversationResponse),
        (status = 401, description = "Non autorisé", body = ErrorResponse),
        (status = 404, description = "Utilisateur introuvable", body = ErrorResponse),
    )
)]
pub async fn get_conversation(
    State(state): State<AppState>,
    Path(other_user_id): Path<i32>,
    Query(params): Query<PaginationParams>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    // Vérification que l'interlocuteur existe
    user::find_by_id(&state.db, other_user_id, &state.encryption_key)
        .await?
        .ok_or(AppError::UserNotFound)?;

    let limit = params.limit.unwrap_or(10).min(50).max(1);

    // Récupération des messages paginés (déchiffrés)
    let messages = message::get_conversation(
        &state.db, claims.sub, other_user_id,
        params.before_id, limit,
        &state.encryption_key,
    ).await?;

    // has_more indique au frontend s'il reste des messages à charger
    let has_more = messages.len() as i64 == limit;

    Ok(Json(json!({
        "conversation_with": other_user_id,
        "messages": messages,
        "has_more": has_more
    })))
}

/// Liste toutes les conversations actives de l'utilisateur connecté.
///
/// Pour chaque conversation, renvoie un aperçu contenant :
/// - L'interlocuteur (id + username)
/// - Le dernier message échangé
/// - La date du dernier message
/// - Le nombre de messages non lus
///
/// Trié du plus récent au plus ancien.
#[utoipa::path(
    get,
    path = "/conversations",
    tag = "Messages",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Liste des conversations", body = ConversationsListResponse),
        (status = 401, description = "Non autorisé", body = ErrorResponse),
    )
)]
pub async fn list_conversations(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    // Requête groupée : dernier message par paire d'utilisateurs (déchiffré)
    let conversations = message::list_conversations(&state.db, claims.sub, &state.encryption_key).await?;

    Ok(Json(json!({
        "conversations": conversations
    })))
}

/// Marque tous les messages d'une conversation comme lus.
///
/// Met à jour `is_read = TRUE` pour tous les messages envoyés par
/// l'interlocuteur au destinataire (l'utilisateur connecté).
#[utoipa::path(
    put,
    path = "/conversations/{user_id}/read",
    tag = "Messages",
    params(("user_id" = i32, Path, description = "ID de l'interlocuteur")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Messages marqués comme lus"),
        (status = 401, description = "Non autorisé", body = ErrorResponse),
    )
)]
pub async fn mark_as_read(
    State(state): State<AppState>,
    Path(other_user_id): Path<i32>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    let count = message::mark_as_read(&state.db, claims.sub, other_user_id).await?;

    Ok(Json(json!({
        "message": "Messages marked as read",
        "count": count
    })))
}
