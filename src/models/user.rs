//! Modèle utilisateur et opérations MongoDB.
//!
//! Contient les structs pour la sérialisation/désérialisation
//! et les fonctions d'accès à la collection `users` dans MongoDB.
//! Les noms d'utilisateur et emails sont chiffrés avec AES-256-GCM.
//! Les lookups utilisent des hash SHA-256 (username_hash, email_hash).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use mongodb::Database;
use bson::doc;
use utoipa::ToSchema;
use futures_util::TryStreamExt;

use crate::crypto;
use crate::errors::AppError;

/// Représentation complète d'un utilisateur en base de données.
/// Le champ `password_hash` est exclu de la sérialisation JSON.
#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    #[serde(alias = "_id")]
    pub id: i32,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[serde(default)]
    pub username_hash: Option<String>,
    #[serde(default)]
    pub email_hash: Option<String>,
    #[serde(default)]
    pub recovery_code_hash: Option<String>,
    /// URL de la photo de profil (stockée dans /uploads/)
    #[serde(default)]
    pub profile_picture_url: Option<String>,
    /// Date de dernière connexion
    #[serde(default, deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional::deserialize")]
    pub last_seen: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
    pub created_at: DateTime<Utc>,
}

/// Corps de requête pour l'inscription d'un nouvel utilisateur.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUser {
    #[schema(example = "alice", min_length = 3, max_length = 50)]
    pub username: String,
    #[schema(example = "alice@example.com")]
    pub email: String,
    #[schema(example = "secret123", min_length = 6)]
    pub password: String,
}

/// Corps de requête pour la connexion
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginUser {
    #[schema(example = "alice")]
    pub username: String,
    #[schema(example = "secret123")]
    pub password: String,
}

/// Corps de requête pour la réinitialisation du mot de passe.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    #[schema(example = "alice")]
    pub username: String,
    #[schema(example = "A1B2C3D4")]
    pub recovery_code: String,
    #[schema(example = "newpassword123", min_length = 6)]
    pub new_password: String,
}

/// Informations utilisateur renvoyées dans les réponses API
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    #[serde(alias = "_id")]
    pub id: i32,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub profile_picture_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default, deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional::deserialize")]
    pub last_seen: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "bson::serde_helpers::chrono_datetime_as_bson_datetime::deserialize")]
    pub created_at: DateTime<Utc>,
}

/// Conversion d'un User (modèle DB) vers UserResponse (réponse API)
impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            profile_picture_url: u.profile_picture_url,
            last_seen: u.last_seen,
            created_at: u.created_at,
        }
    }
}

/// Corps de requête pour la mise à jour du profil
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProfileRequest {
    #[schema(example = "/uploads/abc123.jpg")]
    pub profile_picture_url: Option<String>,
}

/// Déchiffre les champs sensibles d'un User
fn decrypt_user(mut user: User, key: &[u8; 32]) -> User {
    user.username = crypto::try_decrypt(&user.username, key);
    user.email = crypto::try_decrypt(&user.email, key);
    user
}

/// Déchiffre les champs sensibles d'un UserResponse
fn decrypt_user_response(mut user: UserResponse, key: &[u8; 32]) -> UserResponse {
    user.username = crypto::try_decrypt(&user.username, key);
    user.email = crypto::try_decrypt(&user.email, key);
    user
}

/// Insère un nouvel utilisateur et retourne (ID, recovery_code).
pub async fn create_user(
    db: &Database,
    username: &str,
    email: &str,
    password_hash: &str,
    key: &[u8; 32],
) -> Result<(i32, String), AppError> {
    let encrypted_username = crypto::encrypt(username, key).unwrap_or_else(|_| username.to_string());
    let encrypted_email = crypto::encrypt(email, key).unwrap_or_else(|_| email.to_string());
    let username_hash = crypto::hash_value(username);
    let email_hash = crypto::hash_value(email);

    let recovery_code = generate_recovery_code();
    let recovery_code_hash = crypto::hash_value(&recovery_code);

    let new_id = crate::db::next_id(db, "users").await?;

    db.collection::<bson::Document>("users")
        .insert_one(
            doc! {
                "_id": new_id,
                "username": &encrypted_username,
                "email": &encrypted_email,
                "password_hash": password_hash,
                "username_hash": &username_hash,
                "email_hash": &email_hash,
                "recovery_code_hash": &recovery_code_hash,
                "created_at": bson::DateTime::from_chrono(Utc::now()),
            },
            None,
        )
        .await?;

    Ok((new_id, recovery_code))
}

/// Génère un code de récupération aléatoire de 8 caractères (A-Z, 0-9)
fn generate_recovery_code() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// Vérifie le code de récupération d'un utilisateur
pub async fn verify_recovery_code(
    db: &Database,
    user_id: i32,
    recovery_code: &str,
) -> Result<bool, AppError> {
    let code_hash = crypto::hash_value(&recovery_code.to_uppercase());
    let count = db
        .collection::<bson::Document>("users")
        .count_documents(doc! { "_id": user_id, "recovery_code_hash": &code_hash }, None)
        .await?;
    Ok(count > 0)
}

/// Recherche un utilisateur par son nom d'utilisateur (via hash SHA-256).
pub async fn find_by_username(
    db: &Database,
    username: &str,
    key: &[u8; 32],
) -> Result<Option<User>, AppError> {
    let username_hash = crypto::hash_value(username);

    // Chercher par hash (données chiffrées)
    let user = db
        .collection::<User>("users")
        .find_one(doc! { "username_hash": &username_hash }, None)
        .await?;

    if let Some(user) = user {
        return Ok(Some(decrypt_user(user, key)));
    }

    // Fallback : chercher par username en clair (anciennes données)
    let user = db
        .collection::<User>("users")
        .find_one(
            doc! { "username": username, "username_hash": bson::Bson::Null },
            None,
        )
        .await?;

    Ok(user.map(|u| decrypt_user(u, key)))
}

/// Recherche un utilisateur par son ID.
pub async fn find_by_id(
    db: &Database,
    id: i32,
    key: &[u8; 32],
) -> Result<Option<User>, AppError> {
    let user = db
        .collection::<User>("users")
        .find_one(doc! { "_id": id }, None)
        .await?;

    Ok(user.map(|u| decrypt_user(u, key)))
}

/// Met à jour la date de dernière connexion d'un utilisateur.
pub async fn update_last_seen(db: &Database, user_id: i32) -> Result<(), AppError> {
    db.collection::<bson::Document>("users")
        .update_one(
            doc! { "_id": user_id },
            doc! { "$set": { "last_seen": bson::DateTime::from_chrono(Utc::now()) } },
            None,
        )
        .await?;
    Ok(())
}

/// Recherche des utilisateurs par nom (fetch all, déchiffre, filtre en Rust).
pub async fn search_users(
    db: &Database,
    query: &str,
    exclude_user_id: i32,
    key: &[u8; 32],
) -> Result<Vec<UserResponse>, AppError> {
    let cursor = db
        .collection::<UserResponse>("users")
        .find(doc! { "_id": { "$ne": exclude_user_id } }, None)
        .await?;

    let all_users: Vec<UserResponse> = cursor.try_collect().await?;

    let query_lower = query.to_lowercase();
    let results: Vec<UserResponse> = all_users
        .into_iter()
        .map(|u| decrypt_user_response(u, key))
        .filter(|u| u.username.to_lowercase().contains(&query_lower))
        .take(20)
        .collect();

    Ok(results)
}

/// Vérifie si un username ou email est déjà pris.
pub async fn username_or_email_exists(
    db: &Database,
    username: &str,
    email: &str,
) -> Result<bool, AppError> {
    let username_hash = crypto::hash_value(username);
    let email_hash = crypto::hash_value(email);

    // Vérifier par hash
    let count = db
        .collection::<bson::Document>("users")
        .count_documents(
            doc! { "$or": [
                { "username_hash": &username_hash },
                { "email_hash": &email_hash }
            ]},
            None,
        )
        .await?;

    if count > 0 {
        return Ok(true);
    }

    // Fallback : vérifier par valeurs en clair
    let count = db
        .collection::<bson::Document>("users")
        .count_documents(
            doc! { "$or": [
                { "username": username, "username_hash": bson::Bson::Null },
                { "email": email, "email_hash": bson::Bson::Null }
            ]},
            None,
        )
        .await?;

    Ok(count > 0)
}

/// Met à jour le mot de passe d'un utilisateur.
pub async fn update_password(
    db: &Database,
    user_id: i32,
    password_hash: &str,
) -> Result<(), AppError> {
    db.collection::<bson::Document>("users")
        .update_one(
            doc! { "_id": user_id },
            doc! { "$set": { "password_hash": password_hash } },
            None,
        )
        .await?;
    Ok(())
}

/// Met à jour la photo de profil d'un utilisateur.
pub async fn update_profile_picture(
    db: &Database,
    user_id: i32,
    url: Option<&str>,
) -> Result<(), AppError> {
    let value = match url {
        Some(u) => bson::Bson::String(u.to_string()),
        None => bson::Bson::Null,
    };
    db.collection::<bson::Document>("users")
        .update_one(
            doc! { "_id": user_id },
            doc! { "$set": { "profile_picture_url": value } },
            None,
        )
        .await?;
    Ok(())
}
