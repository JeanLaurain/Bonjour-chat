//! Handlers pour la gestion des notifications push.
//!
//! Endpoints pour s'abonner/se désabonner aux notifications push
//! et pour récupérer la clé publique VAPID.
//! L'envoi de push se fait via `send_push_to_user` appelé depuis le handler de messages.

use axum::{
    extract::{Request, State},
    http::header,
    Json,
};
use serde_json::{json, Value};

use crate::config::AppState;
use crate::errors::AppError;
use crate::middleware::auth::{verify_token, Claims};
use crate::models::push_subscription;

/// Extrait les claims JWT depuis le header Authorization
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

/// GET /notifications/vapid-key — Retourne la clé publique VAPID
/// pour que le frontend puisse s'abonner aux notifications push.
/// Endpoint public (pas besoin d'authentification).
pub async fn get_vapid_key(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "public_key": state.vapid_public_key
    }))
}

/// POST /notifications/subscribe — Enregistre un abonnement push.
/// Le frontend envoie l'objet PushSubscription obtenu via pushManager.subscribe().
pub async fn subscribe_push(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

    let payload: push_subscription::SubscribeRequest = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON body".to_string()))?;

    push_subscription::save_subscription(
        &state.db,
        claims.sub,
        &payload.endpoint,
        &payload.keys.p256dh,
        &payload.keys.auth,
    )
    .await?;

    tracing::info!("Push subscription saved for user_id={}", claims.sub);

    Ok(Json(json!({
        "message": "Subscribed to push notifications"
    })))
}

/// POST /notifications/unsubscribe — Supprime un abonnement push.
pub async fn unsubscribe_push(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

    let payload: push_subscription::UnsubscribeRequest = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON body".to_string()))?;

    push_subscription::delete_subscription(&state.db, claims.sub, &payload.endpoint).await?;

    tracing::info!("Push subscription removed for user_id={}", claims.sub);

    Ok(Json(json!({
        "message": "Unsubscribed from push notifications"
    })))
}

/// Envoie une notification push (sans payload) à un utilisateur.
/// Appelée quand un message est envoyé et que le destinataire n'a pas de WebSocket actif.
pub async fn send_push_to_user(state: &AppState, user_id: i32, sender_name: &str) {
    let subscriptions = match push_subscription::get_subscriptions(&state.db, user_id).await {
        Ok(subs) => subs,
        Err(e) => {
            tracing::error!("Failed to get push subscriptions: {:?}", e);
            return;
        }
    };

    if subscriptions.is_empty() {
        return;
    }

    tracing::info!(
        "Sending push to {} subscription(s) for user_id={}",
        subscriptions.len(),
        user_id
    );

    for sub in &subscriptions {
        if let Err(e) = send_single_push(state, sub, sender_name).await {
            tracing::warn!("Push failed for endpoint {}: {:?}", &sub.endpoint[..50.min(sub.endpoint.len())], e);
            // Supprimer les abonnements expirés (410 Gone)
            if format!("{:?}", e).contains("expired") {
                let _ = push_subscription::delete_subscription(&state.db, user_id, &sub.endpoint).await;
            }
        }
    }
}

/// Envoie une notification push individuelle via le protocole Web Push.
/// Utilise VAPID pour l'authentification auprès du service push.
async fn send_single_push(
    state: &AppState,
    subscription: &push_subscription::PushSubscription,
    _sender_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Extraire l'origine de l'endpoint pour le champ "aud" du JWT VAPID
    let aud = extract_origin(&subscription.endpoint)?;

    // Créer le JWT VAPID (ES256)
    let exp = chrono::Utc::now().timestamp() + 86400; // Expire dans 24h
    let claims = json!({
        "aud": aud,
        "exp": exp,
        "sub": "mailto:noreply@bonjour.app"
    });

    let encoding_key = jsonwebtoken::EncodingKey::from_ec_pem(state.vapid_private_pem.as_bytes())?;
    let jwt_header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
    let jwt_token = jsonwebtoken::encode(&jwt_header, &claims, &encoding_key)?;

    // Envoyer la requête push (payload-less)
    let client = reqwest::Client::new();
    let response = client
        .post(&subscription.endpoint)
        .header(
            "Authorization",
            format!("vapid t={}, k={}", jwt_token, state.vapid_public_key),
        )
        .header("TTL", "86400")
        .header("Content-Length", "0")
        .header("Urgency", "high")
        .send()
        .await?;

    let status = response.status();
    if status == reqwest::StatusCode::GONE || status == reqwest::StatusCode::NOT_FOUND {
        return Err("Subscription expired".into());
    }

    if !status.is_success() && status != reqwest::StatusCode::CREATED {
        tracing::warn!("Push service returned status {} for endpoint", status);
    }

    Ok(())
}

/// Extrait l'origine (scheme + host) d'une URL
fn extract_origin(url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url: url::Url = url.parse()?;
    let host = url.host_str().ok_or("No host in push endpoint")?;
    let origin = if let Some(port) = url.port() {
        format!("{}://{}:{}", url.scheme(), host, port)
    } else {
        format!("{}://{}", url.scheme(), host)
    };
    Ok(origin)
}
