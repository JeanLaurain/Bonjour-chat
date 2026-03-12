//! Handlers d'authentification (inscription et connexion).
//!
//! Ces endpoints sont publics (pas de JWT requis).
//! Après inscription ou connexion réussie, un token JWT est retourné
//! au client pour être utilisé dans les requêtes protégées.

use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::config::AppState;
use crate::errors::{AppError, AuthResponse, ErrorResponse};
use crate::middleware::auth::create_token;
use crate::models::user::{self, CreateUser, LoginUser, UserResponse};

/// Inscription d'un nouvel utilisateur.
///
/// Valide les champs (longueur username, format email, longueur password),
/// vérifie l'unicité en base, hash le mot de passe avec bcrypt,
/// puis crée l'utilisateur et retourne un JWT.
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "Authentication",
    request_body = CreateUser,
    responses(
        (status = 200, description = "Utilisateur inscrit avec succès", body = AuthResponse),
        (status = 400, description = "Erreur de validation", body = ErrorResponse),
        (status = 409, description = "Utilisateur déjà existant", body = ErrorResponse),
    )
)]
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<CreateUser>,
) -> Result<Json<Value>, AppError> {
    // Validation du nom d'utilisateur (entre 3 et 50 caractères)
    if payload.username.len() < 3 || payload.username.len() > 50 {
        return Err(AppError::Validation(
            "Username must be between 3 and 50 characters".to_string(),
        ));
    }

    // Validation basique du format email
    if !payload.email.contains('@') {
        return Err(AppError::Validation("Invalid email format".to_string()));
    }

    // Le mot de passe doit faire au moins 6 caractères
    if payload.password.len() < 6 {
        return Err(AppError::Validation(
            "Password must be at least 6 characters".to_string(),
        ));
    }

    // Vérification que le username et l'email ne sont pas déjà pris
    if user::username_or_email_exists(&state.db, &payload.username, &payload.email).await? {
        return Err(AppError::UserAlreadyExists);
    }

    // Hachage du mot de passe avec bcrypt (coût par défaut = 12)
    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)?;

    // Insertion en base de données (username et email chiffrés)
    let user_id = user::create_user(
        &state.db,
        &payload.username,
        &payload.email,
        &password_hash,
        &state.encryption_key,
    )
    .await?;

    // Génération immédiate d'un JWT pour que l'utilisateur soit connecté
    let token = create_token(user_id as i32, &payload.username, &state.jwt_secret)?;

    Ok(Json(json!({
        "message": "User registered successfully",
        "user": {
            "id": user_id,
            "username": payload.username,
            "email": payload.email
        },
        "token": token
    })))
}

/// Connexion d'un utilisateur existant.
///
/// Vérifie le nom d'utilisateur, compare le mot de passe fourni
/// avec le hash bcrypt stocké en base, puis retourne un JWT.
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "Authentication",
    request_body = LoginUser,
    responses(
        (status = 200, description = "Connexion réussie", body = AuthResponse),
        (status = 401, description = "Identifiants invalides", body = ErrorResponse),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginUser>,
) -> Result<Json<Value>, AppError> {
    // Recherche de l'utilisateur par username (lookup par hash)
    let user = user::find_by_username(&state.db, &payload.username, &state.encryption_key)
        .await?
        .ok_or(AppError::InvalidCredentials)?;

    // Vérification du mot de passe avec bcrypt (comparaison sécurisée)
    let is_valid = bcrypt::verify(&payload.password, &user.password_hash)?;
    if !is_valid {
        return Err(AppError::InvalidCredentials);
    }

    // Génération du token JWT
    let token = create_token(user.id, &user.username, &state.jwt_secret)?;

    // Conversion vers UserResponse (sans le password_hash)
    let user_response: UserResponse = user.into();

    Ok(Json(json!({
        "message": "Login successful",
        "user": user_response,
        "token": token
    })))
}
