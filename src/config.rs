//! Configuration globale de l'application.
//!
//! Contient l'état partagé (AppState) injecté dans chaque handler Axum
//! via `State<AppState>`, incluant le pool DB, la clé JWT,
//! et le registre des connexions WebSocket actives.

use sqlx::MySqlPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Type pour le registre des connexions WebSocket actives.
/// Mappe chaque user_id vers une liste de canaux d'envoi (un par onglet/device).
pub type WsConnections = Arc<RwLock<HashMap<i32, Vec<mpsc::UnboundedSender<String>>>>>;

/// État partagé entre tous les handlers.
/// Cloné automatiquement par Axum pour chaque requête.
#[derive(Clone)]
pub struct AppState {
    /// Pool de connexions MySQL (async, géré par sqlx)
    pub db: MySqlPool,
    /// Clé secrète utilisée pour signer et vérifier les tokens JWT
    pub jwt_secret: String,
    /// Registre des connexions WebSocket (temps réel)
    pub ws_connections: WsConnections,
    /// Clé publique VAPID (base64url) pour les notifications push
    pub vapid_public_key: String,
    /// Clé privée VAPID au format PEM pour signer les JWT push
    pub vapid_private_pem: String,
    /// Clé de chiffrement AES-256 pour les données sensibles
    pub encryption_key: [u8; 32],
}
