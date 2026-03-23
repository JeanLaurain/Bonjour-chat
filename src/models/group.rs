//! Modèle de groupe et opérations MongoDB.
//!
//! Gère les groupes de conversation multi-utilisateurs,
//! leurs membres et les messages de groupe.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use mongodb::Database;
use bson::doc;
use utoipa::ToSchema;
use futures_util::TryStreamExt;

use crate::crypto;
use crate::errors::AppError;

/// Représentation d'un groupe en base de données
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Group {
    #[serde(alias = "_id")]
    pub id: i32,
    pub name: String,
    pub creator_id: i32,
    #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
    pub created_at: DateTime<Utc>,
}

/// Membre d'un groupe avec son nom d'utilisateur
#[derive(Debug, Serialize, ToSchema)]
pub struct GroupMember {
    pub user_id: i32,
    pub username: String,
    pub role: String,
}

/// Message dans un groupe avec le nom de l'expéditeur
#[derive(Debug, Serialize, ToSchema)]
pub struct GroupMessage {
    pub id: i32,
    pub group_id: i32,
    pub sender_id: i32,
    pub sender_username: String,
    pub content: String,
    pub message_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Corps de requête pour créer un groupe
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroupRequest {
    pub name: String,
    pub member_ids: Vec<i32>,
}

/// Corps de requête pour envoyer un message dans un groupe
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroupMessage {
    pub content: String,
    #[serde(default = "default_text")]
    pub message_type: String,
    pub image_url: Option<String>,
    pub original_filename: Option<String>,
    pub reply_to_id: Option<i32>,
}

fn default_text() -> String {
    "text".to_string()
}

/// Corps de requête pour ajouter des membres
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMembersRequest {
    pub user_ids: Vec<i32>,
}

/// Corps de requête pour renommer un groupe
#[derive(Debug, Deserialize, ToSchema)]
pub struct RenameGroupRequest {
    pub name: String,
}

/// Aperçu d'un groupe pour la liste des conversations
#[derive(Debug, Serialize, ToSchema)]
pub struct GroupPreview {
    pub id: i32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sender: Option<String>,
    pub member_count: i64,
    pub unread_count: i64,
}

/// Struct interne pour lire un username depuis MongoDB
#[derive(Deserialize)]
struct UserBasic {
    username: String,
}

/// Crée un groupe et ajoute les membres (nom chiffré).
pub async fn create_group(
    db: &Database,
    name: &str,
    creator_id: i32,
    member_ids: &[i32],
    key: &[u8; 32],
) -> Result<i32, AppError> {
    let encrypted_name = crypto::encrypt(name, key).unwrap_or_else(|_| name.to_string());
    let group_id = crate::db::next_id(db, "groups").await?;

    db.collection::<bson::Document>("groups")
        .insert_one(
            doc! {
                "_id": group_id,
                "name": &encrypted_name,
                "creator_id": creator_id,
                "created_at": bson::DateTime::from_chrono(Utc::now()),
            },
            None,
        )
        .await?;

    // Ajouter le créateur comme admin
    db.collection::<bson::Document>("group_members")
        .insert_one(
            doc! { "group_id": group_id, "user_id": creator_id, "role": "admin" },
            None,
        )
        .await?;

    // Ajouter les autres membres
    for &member_id in member_ids {
        if member_id != creator_id {
            db.collection::<bson::Document>("group_members")
                .insert_one(
                    doc! { "group_id": group_id, "user_id": member_id, "role": "member" },
                    None,
                )
                .await?;
        }
    }

    Ok(group_id)
}

/// Récupère les détails d'un groupe et déchiffre le nom
pub async fn get_group(
    db: &Database,
    group_id: i32,
    key: &[u8; 32],
) -> Result<Option<Group>, AppError> {
    let group = db
        .collection::<Group>("groups")
        .find_one(doc! { "_id": group_id }, None)
        .await?;

    Ok(group.map(|mut g| {
        g.name = crypto::try_decrypt(&g.name, key);
        g
    }))
}

/// Vérifie si un utilisateur est membre d'un groupe
pub async fn is_member(db: &Database, group_id: i32, user_id: i32) -> Result<bool, AppError> {
    let count = db
        .collection::<bson::Document>("group_members")
        .count_documents(doc! { "group_id": group_id, "user_id": user_id }, None)
        .await?;
    Ok(count > 0)
}

/// Renomme un groupe (chiffre le nouveau nom)
pub async fn rename_group(
    db: &Database,
    group_id: i32,
    new_name: &str,
    key: &[u8; 32],
) -> Result<(), AppError> {
    let encrypted_name = crypto::encrypt(new_name, key).unwrap_or_else(|_| new_name.to_string());
    db.collection::<bson::Document>("groups")
        .update_one(
            doc! { "_id": group_id },
            doc! { "$set": { "name": &encrypted_name } },
            None,
        )
        .await?;
    Ok(())
}

/// Liste les membres d'un groupe avec leurs noms déchiffrés
pub async fn get_members(
    db: &Database,
    group_id: i32,
    key: &[u8; 32],
) -> Result<Vec<GroupMember>, AppError> {
    #[derive(Deserialize)]
    struct MemberRecord {
        user_id: i32,
        role: String,
    }

    let cursor = db
        .collection::<MemberRecord>("group_members")
        .find(doc! { "group_id": group_id }, None)
        .await?;
    let records: Vec<MemberRecord> = cursor.try_collect().await?;

    let mut members = Vec::new();
    for record in records {
        let user = db
            .collection::<UserBasic>("users")
            .find_one(doc! { "_id": record.user_id }, None)
            .await?;

        let username = match user {
            Some(u) => crypto::try_decrypt(&u.username, key),
            None => "Unknown".to_string(),
        };

        members.push(GroupMember {
            user_id: record.user_id,
            username,
            role: record.role,
        });
    }

    Ok(members)
}

/// Récupère les IDs de tous les membres d'un groupe
pub async fn get_member_ids(db: &Database, group_id: i32) -> Result<Vec<i32>, AppError> {
    #[derive(Deserialize)]
    struct MemberIdOnly {
        user_id: i32,
    }

    let cursor = db
        .collection::<MemberIdOnly>("group_members")
        .find(doc! { "group_id": group_id }, None)
        .await?;
    let records: Vec<MemberIdOnly> = cursor.try_collect().await?;
    Ok(records.into_iter().map(|r| r.user_id).collect())
}

/// Insère un message chiffré dans un groupe et retourne son ID
pub async fn create_group_message(
    db: &Database,
    group_id: i32,
    sender_id: i32,
    content: &str,
    message_type: &str,
    image_url: Option<&str>,
    original_filename: Option<&str>,
    reply_to_id: Option<i32>,
    key: &[u8; 32],
) -> Result<i32, AppError> {
    let encrypted_content = crypto::encrypt(content, key).unwrap_or_else(|_| content.to_string());
    let new_id = crate::db::next_id(db, "group_messages").await?;

    let mut document = doc! {
        "_id": new_id,
        "group_id": group_id,
        "sender_id": sender_id,
        "content": &encrypted_content,
        "message_type": message_type,
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

    db.collection::<bson::Document>("group_messages")
        .insert_one(document, None)
        .await?;

    Ok(new_id)
}

/// Récupère les messages d'un groupe avec pagination (déchiffrés).
pub async fn get_group_messages(
    db: &Database,
    group_id: i32,
    before_id: Option<i32>,
    limit: i64,
    key: &[u8; 32],
) -> Result<Vec<GroupMessage>, AppError> {
    let mut filter = doc! { "group_id": group_id };
    if let Some(bid) = before_id {
        filter.insert("_id", doc! { "$lt": bid });
    }

    let options = mongodb::options::FindOptions::builder()
        .sort(doc! { "_id": -1 })
        .limit(limit)
        .build();

    #[derive(Deserialize)]
    struct RawGroupMessage {
        #[serde(alias = "_id")]
        id: i32,
        group_id: i32,
        sender_id: i32,
        content: String,
        message_type: String,
        #[serde(default)]
        image_url: Option<String>,
        #[serde(default)]
        original_filename: Option<String>,
        #[serde(default)]
        reply_to_id: Option<i32>,
        #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
        created_at: DateTime<Utc>,
    }

    let cursor = db
        .collection::<RawGroupMessage>("group_messages")
        .find(filter, options)
        .await?;
    let raw_messages: Vec<RawGroupMessage> = cursor.try_collect().await?;

    // Batch lookup des noms d'expéditeurs
    let sender_ids: Vec<i32> = raw_messages
        .iter()
        .map(|m| m.sender_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let mut username_map = std::collections::HashMap::new();
    for &sid in &sender_ids {
        if let Some(user) = db
            .collection::<UserBasic>("users")
            .find_one(doc! { "_id": sid }, None)
            .await?
        {
            username_map.insert(sid, crypto::try_decrypt(&user.username, key));
        }
    }

    let mut messages: Vec<GroupMessage> = raw_messages
        .into_iter()
        .map(|m| {
            let sender_username = username_map
                .get(&m.sender_id)
                .cloned()
                .unwrap_or_else(|| "???".to_string());
            GroupMessage {
                id: m.id,
                group_id: m.group_id,
                sender_id: m.sender_id,
                sender_username,
                content: crypto::try_decrypt(&m.content, key),
                message_type: m.message_type,
                image_url: m.image_url,
                original_filename: m.original_filename,
                reply_to_id: m.reply_to_id,
                created_at: m.created_at,
            }
        })
        .collect();

    // Remettre dans l'ordre chronologique
    messages.reverse();

    Ok(messages)
}

/// Liste les groupes d'un utilisateur avec aperçu du dernier message.
pub async fn list_user_groups(
    db: &Database,
    user_id: i32,
    key: &[u8; 32],
) -> Result<Vec<GroupPreview>, AppError> {
    // Étape 1 : récupérer les IDs de groupes de l'utilisateur
    #[derive(Deserialize)]
    struct GroupIdOnly {
        group_id: i32,
    }

    let cursor = db
        .collection::<GroupIdOnly>("group_members")
        .find(doc! { "user_id": user_id }, None)
        .await?;
    let memberships: Vec<GroupIdOnly> = cursor.try_collect().await?;
    let group_ids: Vec<i32> = memberships.into_iter().map(|m| m.group_id).collect();

    if group_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut groups = Vec::new();

    for &gid in &group_ids {
        // Détails du groupe
        let group = match db
            .collection::<Group>("groups")
            .find_one(doc! { "_id": gid }, None)
            .await?
        {
            Some(g) => g,
            None => continue,
        };

        // Dernier message du groupe
        #[derive(Deserialize)]
        struct LastMsg {
            content: String,
            sender_id: i32,
            #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
            created_at: DateTime<Utc>,
        }

        let last_options = mongodb::options::FindOptions::builder()
            .sort(doc! { "_id": -1 })
            .limit(1)
            .build();

        let cursor = db
            .collection::<LastMsg>("group_messages")
            .find(doc! { "group_id": gid }, last_options)
            .await?;
        let last_msgs: Vec<LastMsg> = cursor.try_collect().await?;
        let last_msg = last_msgs.into_iter().next();

        // Nombre de membres
        let member_count = db
            .collection::<bson::Document>("group_members")
            .count_documents(doc! { "group_id": gid }, None)
            .await? as i64;

        // Messages non lus (après le dernier message envoyé par l'utilisateur)
        let my_last_options = mongodb::options::FindOptions::builder()
            .sort(doc! { "created_at": -1 })
            .limit(1)
            .build();

        #[derive(Deserialize)]
        struct MsgTime {
            #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
            created_at: DateTime<Utc>,
        }

        let my_cursor = db
            .collection::<MsgTime>("group_messages")
            .find(
                doc! { "group_id": gid, "sender_id": user_id },
                my_last_options,
            )
            .await?;
        let my_msgs: Vec<MsgTime> = my_cursor.try_collect().await?;
        let my_last_time = my_msgs.into_iter().next().map(|m| m.created_at);

        let unread_filter = if let Some(t) = my_last_time {
            doc! {
                "group_id": gid,
                "sender_id": { "$ne": user_id },
                "created_at": { "$gt": bson::DateTime::from_chrono(t) }
            }
        } else {
            doc! {
                "group_id": gid,
                "sender_id": { "$ne": user_id }
            }
        };

        let unread_count = db
            .collection::<bson::Document>("group_messages")
            .count_documents(unread_filter, None)
            .await? as i64;

        // Nom du dernier expéditeur
        let last_sender = if let Some(ref msg) = last_msg {
            db.collection::<UserBasic>("users")
                .find_one(doc! { "_id": msg.sender_id }, None)
                .await?
                .map(|u| crypto::try_decrypt(&u.username, key))
        } else {
            None
        };

        let (last_message, last_message_at) = match last_msg {
            Some(msg) => (
                Some(crypto::try_decrypt(&msg.content, key)),
                Some(msg.created_at),
            ),
            None => (None, None),
        };

        groups.push(GroupPreview {
            id: gid,
            name: crypto::try_decrypt(&group.name, key),
            last_message,
            last_message_at,
            last_sender,
            member_count,
            unread_count,
        });
    }

    // Trier par dernier message (plus récent d'abord)
    groups.sort_by(|a, b| b.last_message_at.cmp(&a.last_message_at));

    Ok(groups)
}

/// Ajoute des membres à un groupe (ignore les doublons via upsert)
pub async fn add_members(
    db: &Database,
    group_id: i32,
    user_ids: &[i32],
) -> Result<u64, AppError> {
    let mut count = 0u64;
    for &uid in user_ids {
        let options = mongodb::options::UpdateOptions::builder()
            .upsert(true)
            .build();
        let result = db
            .collection::<bson::Document>("group_members")
            .update_one(
                doc! { "group_id": group_id, "user_id": uid },
                doc! { "$setOnInsert": {
                    "group_id": group_id,
                    "user_id": uid,
                    "role": "member"
                }},
                options,
            )
            .await?;
        if result.upserted_id.is_some() {
            count += 1;
        }
    }
    Ok(count)
}

/// Retire un membre d'un groupe
pub async fn remove_member(db: &Database, group_id: i32, user_id: i32) -> Result<(), AppError> {
    db.collection::<bson::Document>("group_members")
        .delete_one(doc! { "group_id": group_id, "user_id": user_id }, None)
        .await?;
    Ok(())
}
