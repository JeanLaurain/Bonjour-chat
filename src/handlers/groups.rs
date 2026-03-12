//! Handlers pour les groupes de conversations.
//!
//! Gère la création de groupes, l'envoi de messages de groupe,
//! la gestion des membres, et la diffusion WebSocket aux membres.

use axum::{
    extract::{Path, Request, State},
    http::header,
    Json,
};
use serde_json::{json, Value};

use crate::config::AppState;
use crate::errors::AppError;
use crate::middleware::auth::{verify_token, Claims};
use crate::models::group;

/// Extrait les claims JWT depuis les headers de la requête
fn extract_claims(req: &Request, jwt_secret: &str) -> Result<Claims, AppError> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;
    verify_token(token, jwt_secret)
}

/// POST /groups — Crée un nouveau groupe de conversation.
/// Le créateur est automatiquement ajouté comme admin.
/// Notifie les membres ajoutés via WebSocket.
pub async fn create_group(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

    let payload: group::CreateGroupRequest = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON body".to_string()))?;

    if payload.name.trim().is_empty() || payload.name.len() > 100 {
        return Err(AppError::Validation(
            "Group name must be between 1 and 100 characters".to_string(),
        ));
    }

    if payload.member_ids.is_empty() {
        return Err(AppError::Validation(
            "A group must have at least one other member".to_string(),
        ));
    }

    let group_id =
        group::create_group(&state.db, &payload.name, claims.sub, &payload.member_ids, &state.encryption_key).await?;
    let members = group::get_members(&state.db, group_id, &state.encryption_key).await?;

    // Notifier les membres via WebSocket qu'un groupe a été créé
    let ws_msg = json!({
        "type": "group_created",
        "data": {
            "group_id": group_id,
            "name": payload.name,
            "creator_id": claims.sub
        }
    })
    .to_string();

    for &member_id in &payload.member_ids {
        crate::handlers::ws::send_to_user(&state, member_id, &ws_msg).await;
    }

    Ok(Json(json!({
        "id": group_id,
        "name": payload.name,
        "members": members
    })))
}

/// GET /groups — Liste tous les groupes de l'utilisateur connecté
pub async fn list_groups(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;
    let groups = group::list_user_groups(&state.db, claims.sub, &state.encryption_key).await?;
    Ok(Json(json!({ "groups": groups })))
}

/// GET /groups/:id — Détails d'un groupe avec la liste des membres
pub async fn get_group(
    State(state): State<AppState>,
    Path(group_id): Path<i32>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    // Vérifier que l'utilisateur est membre du groupe
    if !group::is_member(&state.db, group_id, claims.sub).await? {
        return Err(AppError::Unauthorized);
    }

    let grp = group::get_group(&state.db, group_id, &state.encryption_key)
        .await?
        .ok_or(AppError::UserNotFound)?;
    let members = group::get_members(&state.db, group_id, &state.encryption_key).await?;

    Ok(Json(json!({
        "group": grp,
        "members": members
    })))
}

/// GET /groups/:id/messages — Récupère tous les messages d'un groupe
pub async fn get_group_messages(
    State(state): State<AppState>,
    Path(group_id): Path<i32>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    if !group::is_member(&state.db, group_id, claims.sub).await? {
        return Err(AppError::Unauthorized);
    }

    let messages = group::get_group_messages(&state.db, group_id, &state.encryption_key).await?;
    Ok(Json(json!({ "messages": messages })))
}

/// POST /groups/:id/messages — Envoie un message dans un groupe.
/// Le message est diffusé à tous les membres via WebSocket.
pub async fn send_group_message(
    State(state): State<AppState>,
    Path(group_id): Path<i32>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    if !group::is_member(&state.db, group_id, claims.sub).await? {
        return Err(AppError::Unauthorized);
    }

    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

    let payload: group::CreateGroupMessage = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON body".to_string()))?;

    if payload.message_type == "text"
        && (payload.content.is_empty() || payload.content.len() > 5000)
    {
        return Err(AppError::Validation(
            "Message content must be between 1 and 5000 characters".to_string(),
        ));
    }

    let msg_id = group::create_group_message(
        &state.db,
        group_id,
        claims.sub,
        &payload.content,
        &payload.message_type,
        payload.image_url.as_deref(),
        &state.encryption_key,
    )
    .await?;

    // Récupérer le nom déchiffré de l'expéditeur pour le broadcast
    let sender_name = crate::models::user::find_by_id(&state.db, claims.sub, &state.encryption_key)
        .await
        .ok()
        .flatten()
        .map(|u| u.username)
        .unwrap_or_else(|| "???".to_string());

    // Diffuser le message à tous les membres du groupe
    let ws_msg = json!({
        "type": "new_group_message",
        "data": {
            "id": msg_id,
            "group_id": group_id,
            "sender_id": claims.sub,
            "sender_username": sender_name,
            "content": payload.content,
            "message_type": payload.message_type,
            "image_url": payload.image_url,
            "created_at": chrono::Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string()
        }
    })
    .to_string();

    let member_ids = group::get_member_ids(&state.db, group_id).await?;
    for member_id in &member_ids {
        crate::handlers::ws::send_to_user(&state, *member_id, &ws_msg).await;
    }

    Ok(Json(json!({
        "message": "Message sent to group",
        "id": msg_id
    })))
}

/// POST /groups/:id/members — Ajoute des membres à un groupe
pub async fn add_members(
    State(state): State<AppState>,
    Path(group_id): Path<i32>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    // Seul un membre existant peut ajouter d'autres membres
    if !group::is_member(&state.db, group_id, claims.sub).await? {
        return Err(AppError::Unauthorized);
    }

    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

    let payload: group::AddMembersRequest = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON body".to_string()))?;

    let added = group::add_members(&state.db, group_id, &payload.user_ids).await?;

    // Notifier les nouveaux membres via WebSocket
    let grp = group::get_group(&state.db, group_id, &state.encryption_key).await?;
    if let Some(grp) = grp {
        let ws_msg = json!({
            "type": "group_created",
            "data": {
                "group_id": group_id,
                "name": grp.name,
                "creator_id": grp.creator_id
            }
        })
        .to_string();

        for &uid in &payload.user_ids {
            crate::handlers::ws::send_to_user(&state, uid, &ws_msg).await;
        }
    }

    Ok(Json(json!({
        "message": format!("{} member(s) added", added)
    })))
}

/// DELETE /groups/:id/members/:user_id — Retire un membre d'un groupe.
/// Seul le créateur (admin) ou le membre lui-même peut se retirer.
pub async fn remove_member(
    State(state): State<AppState>,
    Path((group_id, user_id)): Path<(i32, i32)>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    let grp = group::get_group(&state.db, group_id, &state.encryption_key)
        .await?
        .ok_or(AppError::UserNotFound)?;

    // Seul l'admin ou le membre lui-même peut retirer un membre
    if claims.sub != grp.creator_id && claims.sub != user_id {
        return Err(AppError::Unauthorized);
    }

    group::remove_member(&state.db, group_id, user_id).await?;

    Ok(Json(json!({
        "message": "Member removed"
    })))
}
