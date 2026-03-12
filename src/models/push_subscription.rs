//! Modèle d'abonnement push et opérations de base de données.
//!
//! Gère le stockage et la récupération des abonnements Web Push
//! pour l'envoi de notifications quand l'utilisateur est hors ligne.

use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use utoipa::ToSchema;

/// Abonnement push stocké en base de données
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct PushSubscription {
    pub id: i32,
    pub user_id: i32,
    /// URL du service push (ex: https://fcm.googleapis.com/...)
    pub endpoint: String,
    /// Clé publique ECDH du client (base64url)
    pub p256dh: String,
    /// Secret d'authentification du client (base64url)
    pub auth: String,
}

/// Corps de requête pour s'abonner aux notifications push
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubscribeRequest {
    /// URL du service push
    pub endpoint: String,
    /// Clés cryptographiques du client
    pub keys: PushKeys,
}

/// Clés cryptographiques pour l'abonnement push
#[derive(Debug, Deserialize, ToSchema)]
pub struct PushKeys {
    /// Clé publique ECDH (base64url)
    pub p256dh: String,
    /// Secret d'authentification (base64url)
    pub auth: String,
}

/// Corps de requête pour se désabonner
#[derive(Debug, Deserialize, ToSchema)]
pub struct UnsubscribeRequest {
    /// URL du service push à supprimer
    pub endpoint: String,
}

/// Sauvegarde un abonnement push (upsert par endpoint)
pub async fn save_subscription(
    pool: &MySqlPool,
    user_id: i32,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
) -> Result<(), sqlx::Error> {
    // Supprimer l'ancien abonnement pour cet endpoint s'il existe
    sqlx::query("DELETE FROM push_subscriptions WHERE user_id = ? AND endpoint = ?")
        .bind(user_id)
        .bind(endpoint)
        .execute(pool)
        .await?;

    // Insérer le nouvel abonnement
    sqlx::query(
        "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(endpoint)
    .bind(p256dh)
    .bind(auth)
    .execute(pool)
    .await?;

    Ok(())
}

/// Supprime un abonnement push par endpoint
pub async fn delete_subscription(
    pool: &MySqlPool,
    user_id: i32,
    endpoint: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM push_subscriptions WHERE user_id = ? AND endpoint = ?")
        .bind(user_id)
        .bind(endpoint)
        .execute(pool)
        .await?;
    Ok(())
}

/// Récupère tous les abonnements push d'un utilisateur
pub async fn get_subscriptions(
    pool: &MySqlPool,
    user_id: i32,
) -> Result<Vec<PushSubscription>, sqlx::Error> {
    sqlx::query_as::<_, PushSubscription>(
        "SELECT id, user_id, endpoint, p256dh, auth FROM push_subscriptions WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}
