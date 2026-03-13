//! Modèle de groupe et opérations de base de données associées.
//!
//! Gère les groupes de conversation multi-utilisateurs,
//! leurs membres et les messages de groupe.
//! Les noms de groupe et contenus de messages sont chiffrés avec AES-256-GCM.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use utoipa::ToSchema;

use crate::crypto;

/// Représentation d'un groupe en base de données
#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct Group {
    pub id: i32,
    pub name: String,
    pub creator_id: i32,
    pub created_at: NaiveDateTime,
}

/// Membre d'un groupe avec son nom d'utilisateur
#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct GroupMember {
    pub user_id: i32,
    pub username: String,
    pub role: String,
}

/// Message dans un groupe avec le nom de l'expéditeur
#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct GroupMessage {
    pub id: i32,
    pub group_id: i32,
    pub sender_id: i32,
    pub sender_username: String,
    pub content: String,
    pub message_type: String,
    pub image_url: Option<String>,
    pub created_at: NaiveDateTime,
}

/// Corps de requête pour créer un groupe
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroupRequest {
    /// Nom du groupe (1–100 caractères)
    pub name: String,
    /// IDs des membres à ajouter (le créateur est ajouté automatiquement)
    pub member_ids: Vec<i32>,
}

/// Corps de requête pour envoyer un message dans un groupe
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroupMessage {
    pub content: String,
    #[serde(default = "default_text")]
    pub message_type: String,
    pub image_url: Option<String>,
}

fn default_text() -> String {
    "text".to_string()
}

/// Corps de requête pour ajouter des membres à un groupe
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMembersRequest {
    pub user_ids: Vec<i32>,
}

/// Corps de requête pour renommer un groupe
#[derive(Debug, Deserialize, ToSchema)]
pub struct RenameGroupRequest {
    /// Nouveau nom du groupe (1–100 caractères)
    pub name: String,
}

/// Aperçu d'un groupe pour la liste des conversations
#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct GroupPreview {
    pub id: i32,
    pub name: String,
    pub last_message: Option<String>,
    pub last_message_at: Option<NaiveDateTime>,
    pub last_sender: Option<String>,
    pub member_count: i64,
    /// Nombre de messages non lus dans le groupe
    pub unread_count: i64,
}

/// Crée un groupe et ajoute les membres.
/// Le nom du groupe est chiffré avant l'insertion.
pub async fn create_group(
    pool: &MySqlPool,
    name: &str,
    creator_id: i32,
    member_ids: &[i32],
    key: &[u8; 32],
) -> Result<i32, sqlx::Error> {
    let encrypted_name = crypto::encrypt(name, key).unwrap_or_else(|_| name.to_string());

    let result = sqlx::query("INSERT INTO `groups` (name, creator_id) VALUES (?, ?)")
        .bind(&encrypted_name)
        .bind(creator_id)
        .execute(pool)
        .await?;

    let group_id = result.last_insert_id() as i32;

    // Ajouter le créateur comme admin
    sqlx::query("INSERT INTO group_members (group_id, user_id, role) VALUES (?, ?, 'admin')")
        .bind(group_id)
        .bind(creator_id)
        .execute(pool)
        .await?;

    // Ajouter les autres membres
    for &member_id in member_ids {
        if member_id != creator_id {
            sqlx::query(
                "INSERT INTO group_members (group_id, user_id, role) VALUES (?, ?, 'member')",
            )
            .bind(group_id)
            .bind(member_id)
            .execute(pool)
            .await?;
        }
    }

    Ok(group_id)
}

/// Récupère les détails d'un groupe et déchiffre le nom
pub async fn get_group(pool: &MySqlPool, group_id: i32, key: &[u8; 32]) -> Result<Option<Group>, sqlx::Error> {
    let group = sqlx::query_as::<_, Group>(
        "SELECT id, name, creator_id, created_at FROM `groups` WHERE id = ?",
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await?;

    Ok(group.map(|mut g| {
        g.name = crypto::try_decrypt(&g.name, key);
        g
    }))
}

/// Vérifie si un utilisateur est membre d'un groupe
pub async fn is_member(
    pool: &MySqlPool,
    group_id: i32,
    user_id: i32,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM group_members WHERE group_id = ? AND user_id = ?",
    )
    .bind(group_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row > 0)
}

/// Renomme un groupe (chiffre le nouveau nom)
pub async fn rename_group(
    pool: &MySqlPool,
    group_id: i32,
    new_name: &str,
    key: &[u8; 32],
) -> Result<(), sqlx::Error> {
    let encrypted_name = crypto::encrypt(new_name, key).unwrap_or_else(|_| new_name.to_string());
    sqlx::query("UPDATE `groups` SET name = ? WHERE id = ?")
        .bind(&encrypted_name)
        .bind(group_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Liste les membres d'un groupe avec leurs noms d'utilisateur déchiffrés
pub async fn get_members(
    pool: &MySqlPool,
    group_id: i32,
    key: &[u8; 32],
) -> Result<Vec<GroupMember>, sqlx::Error> {
    let mut members = sqlx::query_as::<_, GroupMember>(
        "SELECT gm.user_id, u.username, gm.role \
         FROM group_members gm \
         JOIN users u ON u.id = gm.user_id \
         WHERE gm.group_id = ? \
         ORDER BY gm.role ASC, u.username ASC",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;

    // Déchiffrer les noms d'utilisateur
    for member in &mut members {
        member.username = crypto::try_decrypt(&member.username, key);
    }

    Ok(members)
}

/// Récupère les IDs de tous les membres d'un groupe
pub async fn get_member_ids(pool: &MySqlPool, group_id: i32) -> Result<Vec<i32>, sqlx::Error> {
    sqlx::query_scalar::<_, i32>("SELECT user_id FROM group_members WHERE group_id = ?")
        .bind(group_id)
        .fetch_all(pool)
        .await
}

/// Insère un message chiffré dans un groupe et retourne son ID
pub async fn create_group_message(
    pool: &MySqlPool,
    group_id: i32,
    sender_id: i32,
    content: &str,
    message_type: &str,
    image_url: Option<&str>,
    key: &[u8; 32],
) -> Result<u64, sqlx::Error> {
    let encrypted_content = crypto::encrypt(content, key).unwrap_or_else(|_| content.to_string());

    let result = sqlx::query(
        "INSERT INTO group_messages (group_id, sender_id, content, message_type, image_url) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(group_id)
    .bind(sender_id)
    .bind(&encrypted_content)
    .bind(message_type)
    .bind(image_url)
    .execute(pool)
    .await?;

    Ok(result.last_insert_id())
}

/// Récupère les messages d'un groupe avec pagination.
/// Déchiffre le contenu et les noms d'expéditeur.
pub async fn get_group_messages(
    pool: &MySqlPool,
    group_id: i32,
    before_id: Option<i32>,
    limit: i64,
    key: &[u8; 32],
) -> Result<Vec<GroupMessage>, sqlx::Error> {
    let mut messages = if let Some(bid) = before_id {
        sqlx::query_as::<_, GroupMessage>(
            "SELECT gm.id, gm.group_id, gm.sender_id, u.username AS sender_username, \
             gm.content, gm.message_type, gm.image_url, gm.created_at \
             FROM group_messages gm \
             JOIN users u ON u.id = gm.sender_id \
             WHERE gm.group_id = ? AND gm.id < ? \
             ORDER BY gm.id DESC LIMIT ?",
        )
        .bind(group_id).bind(bid).bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, GroupMessage>(
            "SELECT gm.id, gm.group_id, gm.sender_id, u.username AS sender_username, \
             gm.content, gm.message_type, gm.image_url, gm.created_at \
             FROM group_messages gm \
             JOIN users u ON u.id = gm.sender_id \
             WHERE gm.group_id = ? \
             ORDER BY gm.id DESC LIMIT ?",
        )
        .bind(group_id).bind(limit)
        .fetch_all(pool)
        .await?
    };

    // Remettre dans l'ordre chronologique
    messages.reverse();

    // Déchiffrer le contenu et le nom de l'expéditeur
    for msg in &mut messages {
        msg.content = crypto::try_decrypt(&msg.content, key);
        msg.sender_username = crypto::try_decrypt(&msg.sender_username, key);
    }

    Ok(messages)
}

/// Liste les groupes d'un utilisateur avec aperçu du dernier message.
/// Déchiffre les noms de groupe, messages et noms d'expéditeurs.
/// Inclut le compteur de messages non lus depuis le dernier message vu.
pub async fn list_user_groups(
    pool: &MySqlPool,
    user_id: i32,
    key: &[u8; 32],
) -> Result<Vec<GroupPreview>, sqlx::Error> {
    let mut groups = sqlx::query_as::<_, GroupPreview>(
        "SELECT g.id, g.name, \
         (SELECT gm2.content FROM group_messages gm2 WHERE gm2.group_id = g.id ORDER BY gm2.created_at DESC LIMIT 1) AS last_message, \
         (SELECT gm3.created_at FROM group_messages gm3 WHERE gm3.group_id = g.id ORDER BY gm3.created_at DESC LIMIT 1) AS last_message_at, \
         (SELECT u2.username FROM group_messages gm4 JOIN users u2 ON u2.id = gm4.sender_id WHERE gm4.group_id = g.id ORDER BY gm4.created_at DESC LIMIT 1) AS last_sender, \
         (SELECT COUNT(*) FROM group_members gm5 WHERE gm5.group_id = g.id) AS member_count, \
         (SELECT COUNT(*) FROM group_messages gm6 WHERE gm6.group_id = g.id AND gm6.sender_id != ? \
          AND gm6.created_at > COALESCE( \
              (SELECT MAX(gm7.created_at) FROM group_messages gm7 WHERE gm7.group_id = g.id AND gm7.sender_id = ?), \
              '1970-01-01' \
          )) AS unread_count \
         FROM `groups` g \
         JOIN group_members gm ON gm.group_id = g.id AND gm.user_id = ? \
         ORDER BY COALESCE( \
             (SELECT gm8.created_at FROM group_messages gm8 WHERE gm8.group_id = g.id ORDER BY gm8.created_at DESC LIMIT 1), \
             g.created_at \
         ) DESC",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // Déchiffrer le nom du groupe, le dernier message et le nom de l'expéditeur
    for group in &mut groups {
        group.name = crypto::try_decrypt(&group.name, key);
        if let Some(ref msg) = group.last_message {
            group.last_message = Some(crypto::try_decrypt(msg, key));
        }
        if let Some(ref sender) = group.last_sender {
            group.last_sender = Some(crypto::try_decrypt(sender, key));
        }
    }

    Ok(groups)
}

/// Ajoute des membres à un groupe (ignore les doublons)
pub async fn add_members(
    pool: &MySqlPool,
    group_id: i32,
    user_ids: &[i32],
) -> Result<u64, sqlx::Error> {
    let mut count = 0u64;
    for &uid in user_ids {
        let result = sqlx::query(
            "INSERT IGNORE INTO group_members (group_id, user_id, role) VALUES (?, ?, 'member')",
        )
        .bind(group_id)
        .bind(uid)
        .execute(pool)
        .await?;
        count += result.rows_affected();
    }
    Ok(count)
}

/// Retire un membre d'un groupe
pub async fn remove_member(
    pool: &MySqlPool,
    group_id: i32,
    user_id: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM group_members WHERE group_id = ? AND user_id = ?")
        .bind(group_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
