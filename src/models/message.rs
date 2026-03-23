//! Modèle de message et opérations MongoDB.
//!
//! Gère les messages directs entre utilisateurs : création, récupération
//! d'une conversation, et listing de toutes les conversations actives.
//! Les contenus de messages sont chiffrés avec AES-256-GCM.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use mongodb::Database;
use bson::doc;
use utoipa::ToSchema;
use futures_util::TryStreamExt;

use crate::crypto;
use crate::errors::AppError;

/// Représentation d'un message en base de données
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Message {
    #[serde(alias = "_id")]
    pub id: i32,
    pub sender_id: i32,
    pub receiver_id: i32,
    pub content: String,
    pub message_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<i32>,
    #[serde(default)]
    pub is_read: bool,
    #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
    pub created_at: DateTime<Utc>,
}

/// Corps de requête pour envoyer un nouveau message
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMessage {
    #[schema(example = 2)]
    pub receiver_id: i32,
    #[schema(example = "Bonjour!")]
    pub content: String,
    #[serde(default = "default_text")]
    #[schema(example = "text")]
    pub message_type: String,
    pub image_url: Option<String>,
    pub original_filename: Option<String>,
    pub reply_to_id: Option<i32>,
}

fn default_text() -> String {
    "text".to_string()
}

/// Aperçu d'une conversation pour la liste
#[derive(Debug, Serialize, ToSchema)]
pub struct ConversationPreview {
    pub user_id: i32,
    pub username: String,
    pub last_message: String,
    pub last_message_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<DateTime<Utc>>,
    pub unread_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_picture_url: Option<String>,
}

/// Insère un nouveau message (contenu chiffré). Retourne l'ID.
pub async fn create_message(
    db: &Database,
    sender_id: i32,
    receiver_id: i32,
    content: &str,
    message_type: &str,
    image_url: Option<&str>,
    original_filename: Option<&str>,
    reply_to_id: Option<i32>,
    key: &[u8; 32],
) -> Result<i32, AppError> {
    let encrypted_content = crypto::encrypt(content, key).unwrap_or_else(|_| content.to_string());
    let new_id = crate::db::next_id(db, "messages").await?;

    let mut document = doc! {
        "_id": new_id,
        "sender_id": sender_id,
        "receiver_id": receiver_id,
        "content": &encrypted_content,
        "message_type": message_type,
        "is_read": false,
        "created_at": bson::DateTime::from_chrono(Utc::now()),
    };

    if let Some(url) = image_url {
        document.insert("image_url", url);
    }
    if let Some(name) = original_filename {
        document.insert("original_filename", name);
    }
    if let Some(reply_id) = reply_to_id {
        document.insert("reply_to_id", reply_id);
    }

    db.collection::<bson::Document>("messages")
        .insert_one(document, None)
        .await?;

    Ok(new_id)
}

/// Récupère les messages échangés entre deux utilisateurs avec pagination.
pub async fn get_conversation(
    db: &Database,
    user_id: i32,
    other_user_id: i32,
    before_id: Option<i32>,
    limit: i64,
    key: &[u8; 32],
) -> Result<Vec<Message>, AppError> {
    let mut filter = doc! {
        "$or": [
            { "sender_id": user_id, "receiver_id": other_user_id },
            { "sender_id": other_user_id, "receiver_id": user_id }
        ]
    };

    if let Some(bid) = before_id {
        filter.insert("_id", doc! { "$lt": bid });
    }

    let options = mongodb::options::FindOptions::builder()
        .sort(doc! { "_id": -1 })
        .limit(limit)
        .build();

    let cursor = db
        .collection::<Message>("messages")
        .find(filter, options)
        .await?;

    let mut messages: Vec<Message> = cursor.try_collect().await?;

    // Remettre dans l'ordre chronologique
    messages.reverse();

    // Déchiffrer le contenu de chaque message
    for msg in &mut messages {
        msg.content = crypto::try_decrypt(&msg.content, key);
    }

    Ok(messages)
}

/// Liste toutes les conversations actives de l'utilisateur.
pub async fn list_conversations(
    db: &Database,
    user_id: i32,
    key: &[u8; 32],
) -> Result<Vec<ConversationPreview>, AppError> {
    // Pipeline d'agrégation pour obtenir le dernier message par conversation
    let pipeline = vec![
        doc! { "$match": {
            "$or": [
                { "sender_id": user_id },
                { "receiver_id": user_id }
            ]
        }},
        doc! { "$addFields": {
            "partner_id": {
                "$cond": {
                    "if": { "$eq": ["$sender_id", user_id] },
                    "then": "$receiver_id",
                    "else": "$sender_id"
                }
            }
        }},
        doc! { "$sort": { "_id": -1 } },
        doc! { "$group": {
            "_id": "$partner_id",
            "last_message": { "$first": "$content" },
            "last_message_at": { "$first": "$created_at" },
        }},
        doc! { "$sort": { "last_message_at": -1 } },
    ];

    let mut cursor = db
        .collection::<bson::Document>("messages")
        .aggregate(pipeline, None)
        .await?;

    let mut conversations = Vec::new();

    while let Some(doc_result) = cursor.try_next().await? {
        let partner_id = doc_result.get_i32("_id").unwrap_or(0);
        let last_message = doc_result.get_str("last_message").unwrap_or("").to_string();
        let last_message_at = doc_result
            .get_datetime("last_message_at")
            .map(|dt| dt.to_chrono())
            .unwrap_or_else(|_| Utc::now());

        // Récupérer les infos du partenaire
        let user_opt = db
            .collection::<super::user::User>("users")
            .find_one(doc! { "_id": partner_id }, None)
            .await?;

        let (username, last_seen, profile_picture_url) = match user_opt {
            Some(u) => {
                let decrypted = super::user::User {
                    username: crypto::try_decrypt(&u.username, key),
                    email: crypto::try_decrypt(&u.email, key),
                    ..u
                };
                (
                    decrypted.username,
                    decrypted.last_seen,
                    decrypted.profile_picture_url,
                )
            }
            None => ("Unknown".to_string(), None, None),
        };

        // Compter les messages non lus de ce partenaire
        let unread_count = db
            .collection::<bson::Document>("messages")
            .count_documents(
                doc! {
                    "sender_id": partner_id,
                    "receiver_id": user_id,
                    "is_read": false,
                },
                None,
            )
            .await? as i64;

        conversations.push(ConversationPreview {
            user_id: partner_id,
            username,
            last_message: crypto::try_decrypt(&last_message, key),
            last_message_at,
            last_seen,
            unread_count,
            profile_picture_url,
        });
    }

    Ok(conversations)
}

/// Marque tous les messages d'un expéditeur comme lus pour le destinataire.
pub async fn mark_as_read(
    db: &Database,
    receiver_id: i32,
    sender_id: i32,
) -> Result<u64, AppError> {
    let result = db
        .collection::<bson::Document>("messages")
        .update_many(
            doc! {
                "sender_id": sender_id,
                "receiver_id": receiver_id,
                "is_read": false,
            },
            doc! { "$set": { "is_read": true } },
            None,
        )
        .await?;

    Ok(result.modified_count)
}
