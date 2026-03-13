//! Modèle de message et opérations de base de données associées.
//!
//! Gère les messages directs entre utilisateurs : création, récupération
//! d'une conversation, et listing de toutes les conversations actives.
//! Les contenus de messages sont chiffrés avec AES-256-GCM.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use utoipa::ToSchema;

use crate::crypto;

/// Représentation d'un message en base de données
#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct Message {
    /// Identifiant unique du message
    pub id: i32,
    /// ID de l'expéditeur
    pub sender_id: i32,
    /// ID du destinataire
    pub receiver_id: i32,
    /// Contenu textuel du message
    pub content: String,
    /// Type de message : "text" ou "image"
    pub message_type: String,
    /// URL de l'image (si message_type = "image")
    pub image_url: Option<String>,
    /// Indique si le message a été lu par le destinataire
    pub is_read: bool,
    /// Date et heure d'envoi
    pub created_at: NaiveDateTime,
}

/// Corps de requête pour envoyer un nouveau message
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMessage {
    /// ID de l'utilisateur destinataire
    #[schema(example = 2)]
    pub receiver_id: i32,
    /// Contenu du message (texte ou légende de l'image)
    #[schema(example = "Bonjour!")]
    pub content: String,
    /// Type de message : "text" (défaut) ou "image"
    #[serde(default = "default_text")]
    #[schema(example = "text")]
    pub message_type: String,
    /// URL de l'image (requis si message_type = "image")
    pub image_url: Option<String>,
}

/// Valeur par défaut pour le type de message
fn default_text() -> String {
    "text".to_string()
}

/// Aperçu d'une conversation : affiche le dernier message échangé
/// avec un utilisateur donné (utilisé dans la liste des conversations)
#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct ConversationPreview {
    /// ID de l'interlocuteur
    pub user_id: i32,
    /// Nom d'utilisateur de l'interlocuteur
    pub username: String,
    /// Contenu du dernier message
    pub last_message: String,
    /// Date du dernier message
    pub last_message_at: NaiveDateTime,
    /// Dernière connexion de l'interlocuteur (null = jamais ou en ligne)
    pub last_seen: Option<NaiveDateTime>,
    /// Nombre de messages non lus de cet interlocuteur
    pub unread_count: i64,
}

/// Insère un nouveau message en base de données.
/// Le contenu du message est chiffré avant l'insertion.
pub async fn create_message(
    pool: &MySqlPool,
    sender_id: i32,
    receiver_id: i32,
    content: &str,
    message_type: &str,
    image_url: Option<&str>,
    key: &[u8; 32],
) -> Result<u64, sqlx::Error> {
    let encrypted_content = crypto::encrypt(content, key).unwrap_or_else(|_| content.to_string());

    let result = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, content, message_type, image_url) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(sender_id)
    .bind(receiver_id)
    .bind(&encrypted_content)
    .bind(message_type)
    .bind(image_url)
    .execute(pool)
    .await?;

    Ok(result.last_insert_id())
}

/// Récupère les messages échangés entre deux utilisateurs avec pagination.
/// Si `before_id` est fourni, retourne les messages antérieurs à cet ID.
/// `limit` contrôle le nombre de messages retournés (défaut 10).
pub async fn get_conversation(
    pool: &MySqlPool,
    user_id: i32,
    other_user_id: i32,
    before_id: Option<i32>,
    limit: i64,
    key: &[u8; 32],
) -> Result<Vec<Message>, sqlx::Error> {
    let mut messages = if let Some(bid) = before_id {
        // Charger les messages avant un ID donné (scroll vers le haut)
        sqlx::query_as::<_, Message>(
            "SELECT id, sender_id, receiver_id, content, message_type, image_url, is_read, created_at FROM messages \
             WHERE ((sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?)) AND id < ? \
             ORDER BY id DESC LIMIT ?"
        )
        .bind(user_id).bind(other_user_id)
        .bind(other_user_id).bind(user_id)
        .bind(bid).bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        // Charger les N derniers messages
        sqlx::query_as::<_, Message>(
            "SELECT id, sender_id, receiver_id, content, message_type, image_url, is_read, created_at FROM messages \
             WHERE (sender_id = ? AND receiver_id = ?) OR (sender_id = ? AND receiver_id = ?) \
             ORDER BY id DESC LIMIT ?"
        )
        .bind(user_id).bind(other_user_id)
        .bind(other_user_id).bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    // Remettre dans l'ordre chronologique (ASC) après le LIMIT DESC
    messages.reverse();

    // Déchiffrer le contenu de chaque message
    for msg in &mut messages {
        msg.content = crypto::try_decrypt(&msg.content, key);
    }

    Ok(messages)
}

/// Liste toutes les conversations actives de l'utilisateur.
/// Déchiffre le nom d'utilisateur et le dernier message.
pub async fn list_conversations(
    pool: &MySqlPool,
    user_id: i32,
    key: &[u8; 32],
) -> Result<Vec<ConversationPreview>, sqlx::Error> {
    let mut conversations = sqlx::query_as::<_, ConversationPreview>(
        "SELECT u.id AS user_id, u.username, m.content AS last_message, m.created_at AS last_message_at, u.last_seen, \
         (SELECT COUNT(*) FROM messages m3 WHERE m3.sender_id = u.id AND m3.receiver_id = ? AND m3.is_read = FALSE) AS unread_count \
         FROM messages m \
         JOIN users u ON u.id = CASE WHEN m.sender_id = ? THEN m.receiver_id ELSE m.sender_id END \
         WHERE m.id IN ( \
             SELECT MAX(m2.id) FROM messages m2 \
             WHERE m2.sender_id = ? OR m2.receiver_id = ? \
             GROUP BY LEAST(m2.sender_id, m2.receiver_id), GREATEST(m2.sender_id, m2.receiver_id) \
         ) \
         ORDER BY m.created_at DESC"
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // Déchiffrer le nom d'utilisateur et le dernier message
    for conv in &mut conversations {
        conv.username = crypto::try_decrypt(&conv.username, key);
        conv.last_message = crypto::try_decrypt(&conv.last_message, key);
    }

    Ok(conversations)
}

/// Marque tous les messages d'un expéditeur comme lus pour le destinataire.
/// Retourne le nombre de messages marqués comme lus.
pub async fn mark_as_read(
    pool: &MySqlPool,
    receiver_id: i32,
    sender_id: i32,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE messages SET is_read = TRUE WHERE sender_id = ? AND receiver_id = ? AND is_read = FALSE",
    )
    .bind(sender_id)
    .bind(receiver_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
