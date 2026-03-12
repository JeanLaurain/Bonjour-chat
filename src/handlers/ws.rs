//! Handler WebSocket pour les notifications temps réel.
//!
//! Les clients se connectent via `/ws?token=<jwt>`.
//! Les messages sont poussés en temps réel sans polling.
//! Le statut en ligne/hors ligne est diffusé aux autres utilisateurs.

use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::config::AppState;
use crate::middleware::auth::verify_token;
use crate::models::user;

/// Paramètres de connexion WebSocket (token JWT dans l'URL)
#[derive(Debug, Deserialize)]
pub struct WsParams {
    /// Token JWT pour authentifier la connexion
    pub token: String,
}

/// Endpoint de connexion WebSocket : GET /ws?token=<jwt>
/// Vérifie le token puis upgrade la connexion HTTP en WebSocket.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match verify_token(&params.token, &state.jwt_secret) {
        Ok(claims) => {
            ws.on_upgrade(move |socket| handle_socket(socket, claims.sub, state))
        }
        Err(_) => {
            // Token invalide : accepter puis fermer immédiatement
            ws.on_upgrade(|mut socket| async move {
                let _ = socket.send(WsMessage::Close(None)).await;
            })
        }
    }
}

/// Gère une connexion WebSocket authentifiée.
/// Enregistre le client, transmet les messages en temps réel,
/// et met à jour le statut "dernière connexion" à la déconnexion.
async fn handle_socket(socket: WebSocket, user_id: i32, state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Enregistrer la connexion dans le registre global
    {
        let mut conns = state.ws_connections.write().await;
        conns.entry(user_id).or_default().push(tx);
    }

    // Notifier que l'utilisateur est en ligne
    broadcast_status(&state, user_id, true).await;

    tracing::info!("WebSocket connecté : user_id={}", user_id);

    // Tâche d'envoi : canal interne → WebSocket client
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender
                .send(WsMessage::Text(msg.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Tâche de réception : WebSocket client → détection de fermeture
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if matches!(msg, WsMessage::Close(_)) {
                break;
            }
        }
    });

    // Attendre que l'une des tâches se termine, puis annuler l'autre
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Mettre à jour la date de dernière connexion
    let _ = user::update_last_seen(&state.db, user_id).await;

    // Notifier que l'utilisateur est hors ligne
    broadcast_status(&state, user_id, false).await;

    tracing::info!("WebSocket déconnecté : user_id={}", user_id);
}

/// Envoie un message JSON à un utilisateur via ses connexions WebSocket.
/// Nettoie automatiquement les connexions mortes (lazy cleanup).
pub async fn send_to_user(state: &AppState, user_id: i32, message: &str) {
    let mut conns = state.ws_connections.write().await;
    if let Some(senders) = conns.get_mut(&user_id) {
        senders.retain(|tx| tx.send(message.to_string()).is_ok());
        if senders.is_empty() {
            conns.remove(&user_id);
        }
    }
}

/// Diffuse le statut en ligne/hors ligne à tous les utilisateurs connectés
async fn broadcast_status(state: &AppState, user_id: i32, online: bool) {
    let status_msg = serde_json::json!({
        "type": "user_status",
        "data": {
            "user_id": user_id,
            "online": online
        }
    })
    .to_string();

    let conns = state.ws_connections.read().await;
    for (&uid, senders) in conns.iter() {
        if uid != user_id {
            for tx in senders {
                let _ = tx.send(status_msg.clone());
            }
        }
    }
}
