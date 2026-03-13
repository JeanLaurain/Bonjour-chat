//! Modèle utilisateur et opérations de base de données associées.
//!
//! Contient les structs pour la sérialisation/désérialisation
//! et les fonctions d'accès à la table `users` dans MySQL.
//! Les noms d'utilisateur et emails sont chiffrés avec AES-256-GCM.
//! Les lookups utilisent des hash SHA-256 (username_hash, email_hash).

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use utoipa::ToSchema;

use crate::crypto;

/// Représentation complète d'un utilisateur en base de données.
/// Le champ `password_hash` est exclu de la sérialisation JSON
/// pour ne jamais le renvoyer au client.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// Date de dernière connexion (null si jamais connecté via WebSocket)
    pub last_seen: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

/// Corps de requête pour l'inscription d'un nouvel utilisateur.
/// Les contraintes de validation sont appliquées dans le handler.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUser {
    /// Nom d'utilisateur unique (entre 3 et 50 caractères)
    #[schema(example = "alice", min_length = 3, max_length = 50)]
    pub username: String,
    /// Adresse email valide (doit contenir @)
    #[schema(example = "alice@example.com")]
    pub email: String,
    /// Mot de passe en clair (minimum 6 caractères, hashé en bcrypt côté serveur)
    #[schema(example = "secret123", min_length = 6)]
    pub password: String,
}

/// Corps de requête pour la connexion
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginUser {
    /// Nom d'utilisateur
    #[schema(example = "alice")]
    pub username: String,
    /// Mot de passe en clair
    #[schema(example = "secret123")]
    pub password: String,
}

/// Corps de requête pour la réinitialisation du mot de passe.
/// L'utilisateur doit fournir son username et email pour prouver qu'il est le propriétaire.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetPasswordRequest {
    /// Nom d'utilisateur du compte
    #[schema(example = "alice")]
    pub username: String,
    /// Email associé au compte (doit correspondre)
    #[schema(example = "alice@example.com")]
    pub email: String,
    /// Nouveau mot de passe (minimum 6 caractères)
    #[schema(example = "newpassword123", min_length = 6)]
    pub new_password: String,
}

/// Informations utilisateur renvoyées dans les réponses API
/// (sans le hash du mot de passe)
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub last_seen: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

/// Conversion d'un User (modèle DB) vers UserResponse (réponse API)
impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            last_seen: u.last_seen,
            created_at: u.created_at,
        }
    }
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

/// Insère un nouvel utilisateur en base et retourne son ID auto-incrémenté.
/// Le username et l'email sont chiffrés ; les hash sont stockés pour les lookups.
pub async fn create_user(
    pool: &MySqlPool,
    username: &str,
    email: &str,
    password_hash: &str,
    key: &[u8; 32],
) -> Result<u64, sqlx::Error> {
    let encrypted_username = crypto::encrypt(username, key).unwrap_or_else(|_| username.to_string());
    let encrypted_email = crypto::encrypt(email, key).unwrap_or_else(|_| email.to_string());
    let username_hash = crypto::hash_value(username);
    let email_hash = crypto::hash_value(email);

    let result = sqlx::query(
        "INSERT INTO users (username, email, password_hash, username_hash, email_hash) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&encrypted_username)
    .bind(&encrypted_email)
    .bind(password_hash)
    .bind(&username_hash)
    .bind(&email_hash)
    .execute(pool)
    .await?;

    Ok(result.last_insert_id())
}

/// Recherche un utilisateur par son nom d'utilisateur.
/// Utilise le hash SHA-256 pour le lookup, avec fallback en clair pour la rétrocompatibilité.
pub async fn find_by_username(
    pool: &MySqlPool,
    username: &str,
    key: &[u8; 32],
) -> Result<Option<User>, sqlx::Error> {
    let username_hash = crypto::hash_value(username);

    // D'abord chercher par hash (données chiffrées)
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, last_seen, created_at FROM users WHERE username_hash = ?"
    )
    .bind(&username_hash)
    .fetch_optional(pool)
    .await?;

    if let Some(user) = user {
        return Ok(Some(decrypt_user(user, key)));
    }

    // Fallback : chercher par username en clair (anciennes données non chiffrées)
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, last_seen, created_at FROM users WHERE username = ? AND username_hash IS NULL"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(user.map(|u| decrypt_user(u, key)))
}

/// Recherche un utilisateur par son ID.
/// Utilisé pour vérifier l'existence d'un destinataire avant d'envoyer un message.
pub async fn find_by_id(
    pool: &MySqlPool,
    id: i32,
    key: &[u8; 32],
) -> Result<Option<User>, sqlx::Error> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, last_seen, created_at FROM users WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(user.map(|u| decrypt_user(u, key)))
}

/// Met à jour la date de dernière connexion d'un utilisateur.
/// Appelé automatiquement lors de la déconnexion WebSocket.
pub async fn update_last_seen(
    pool: &MySqlPool,
    user_id: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_seen = NOW() WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Recherche des utilisateurs par nom (fetch all, déchiffre, filtre en Rust).
/// On ne peut pas utiliser LIKE sur des données chiffrées, donc on récupère
/// tous les utilisateurs, on déchiffre les noms, et on filtre localement.
pub async fn search_users(
    pool: &MySqlPool,
    query: &str,
    exclude_user_id: i32,
    key: &[u8; 32],
) -> Result<Vec<UserResponse>, sqlx::Error> {
    let all_users = sqlx::query_as::<_, UserResponse>(
        "SELECT id, username, email, last_seen, created_at FROM users WHERE id != ?"
    )
    .bind(exclude_user_id)
    .fetch_all(pool)
    .await?;

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
/// Utilise les hash pour la comparaison, avec fallback en clair.
pub async fn username_or_email_exists(
    pool: &MySqlPool,
    username: &str,
    email: &str,
) -> Result<bool, sqlx::Error> {
    let username_hash = crypto::hash_value(username);
    let email_hash = crypto::hash_value(email);

    // Vérifier par hash (données chiffrées)
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM users WHERE username_hash = ? OR email_hash = ?"
    )
    .bind(&username_hash)
    .bind(&email_hash)
    .fetch_one(pool)
    .await?;

    if row.0 > 0 {
        return Ok(true);
    }

    // Fallback : vérifier par valeurs en clair (anciennes données)
    let row2: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM users WHERE (username = ? AND username_hash IS NULL) OR (email = ? AND email_hash IS NULL)"
    )
    .bind(username)
    .bind(email)
    .fetch_one(pool)
    .await?;

    Ok(row2.0 > 0)
}

/// Met à jour le mot de passe (hash bcrypt) d'un utilisateur.
pub async fn update_password(
    pool: &MySqlPool,
    user_id: i32,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
