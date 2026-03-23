//! Modèle d'abonnement push et opérations MongoDB.

use serde::{Deserialize, Serialize};
use mongodb::Database;
use bson::doc;
use utoipa::ToSchema;
use futures_util::TryStreamExt;

use crate::errors::AppError;

/// Abonnement push stocké en base de données
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PushSubscription {
    #[serde(alias = "_id")]
    pub id: i32,
    pub user_id: i32,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

/// Corps de requête pour s'abonner aux notifications push
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubscribeRequest {
    pub endpoint: String,
    pub keys: PushKeys,
}

/// Clés cryptographiques pour l'abonnement push
#[derive(Debug, Deserialize, ToSchema)]
pub struct PushKeys {
    pub p256dh: String,
    pub auth: String,
}

/// Corps de requête pour se désabonner
#[derive(Debug, Deserialize, ToSchema)]
pub struct UnsubscribeRequest {
    pub endpoint: String,
}

/// Sauvegarde un abonnement push (upsert par endpoint)
pub async fn save_subscription(
    db: &Database,
    user_id: i32,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
) -> Result<(), AppError> {
    // Supprimer l'ancien abonnement pour cet endpoint
    db.collection::<bson::Document>("push_subscriptions")
        .delete_many(doc! { "user_id": user_id, "endpoint": endpoint }, None)
        .await?;

    // Insérer le nouvel abonnement
    let new_id = crate::db::next_id(db, "push_subscriptions").await?;
    db.collection::<bson::Document>("push_subscriptions")
        .insert_one(
            doc! {
                "_id": new_id,
                "user_id": user_id,
                "endpoint": endpoint,
                "p256dh": p256dh,
                "auth": auth,
            },
            None,
        )
        .await?;

    Ok(())
}

/// Supprime un abonnement push par endpoint
pub async fn delete_subscription(
    db: &Database,
    user_id: i32,
    endpoint: &str,
) -> Result<(), AppError> {
    db.collection::<bson::Document>("push_subscriptions")
        .delete_many(doc! { "user_id": user_id, "endpoint": endpoint }, None)
        .await?;
    Ok(())
}

/// Récupère tous les abonnements push d'un utilisateur
pub async fn get_subscriptions(
    db: &Database,
    user_id: i32,
) -> Result<Vec<PushSubscription>, AppError> {
    let cursor = db
        .collection::<PushSubscription>("push_subscriptions")
        .find(doc! { "user_id": user_id }, None)
        .await?;
    let subs: Vec<PushSubscription> = cursor.try_collect().await?;
    Ok(subs)
}
