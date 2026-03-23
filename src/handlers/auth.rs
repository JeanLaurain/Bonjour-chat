//! Handlers d'authentification (inscription, connexion, refresh, logout, profil).
//!
//! Les endpoints register, login, reset-password et refresh sont publics.
//! Les endpoints de profil (get_me, update_profile) et logout nécessitent un JWT.

use axum::{extract::{Request, State}, Json};
use axum::http::header;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::AppState;
use crate::errors::{AppError, AuthResponse, ErrorResponse};
use crate::middleware::auth::{
    create_token, verify_token, Claims,
    create_refresh_token, verify_and_rotate_refresh_token, delete_all_refresh_tokens,
};
use crate::models::user::{self, CreateUser, LoginUser, ResetPasswordRequest, UpdateProfileRequest, UserResponse};

/// Extrait les claims JWT depuis le header Authorization de la requête.
fn extract_claims(req: &Request, jwt_secret: &str) -> Result<Claims, AppError> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;
    verify_token(token, jwt_secret)
}

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
    let (user_id, recovery_code) = user::create_user(
        &state.db,
        &payload.username,
        &payload.email,
        &password_hash,
        &state.encryption_key,
    )
    .await?;

    // Génération immédiate d'un JWT pour que l'utilisateur soit connecté
    let token = create_token(user_id as i32, &payload.username, &state.jwt_secret)?;

    // Génération du refresh token (7 jours) pour renouveler la session
    let refresh_token = create_refresh_token(&state.db, user_id as i32).await?;

    Ok(Json(json!({
        "message": "User registered successfully",
        "user": {
            "id": user_id,
            "username": payload.username,
            "email": payload.email,
            "profile_picture_url": Option::<String>::None
        },
        "token": token,
        "refresh_token": refresh_token,
        "recovery_code": recovery_code
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

    // Génération du refresh token (7 jours)
    let refresh_token = create_refresh_token(&state.db, user.id).await?;

    // Conversion vers UserResponse (sans le password_hash)
    let user_response: UserResponse = user.into();

    Ok(Json(json!({
        "message": "Login successful",
        "user": user_response,
        "token": token,
        "refresh_token": refresh_token
    })))
}

/// Réinitialisation du mot de passe via code de récupération.
///
/// L'utilisateur fournit son username et le code de récupération
/// qu'il a reçu lors de son inscription. Ce code est secret et
/// unique à chaque utilisateur, contrairement à l'email qui peut
/// être deviné.
#[utoipa::path(
    post,
    path = "/auth/reset-password",
    tag = "Authentication",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Mot de passe réinitialisé"),
        (status = 400, description = "Erreur de validation", body = ErrorResponse),
        (status = 401, description = "Code de récupération invalide", body = ErrorResponse),
        (status = 404, description = "Utilisateur introuvable", body = ErrorResponse),
    )
)]
pub async fn reset_password(
    State(state): State<AppState>,
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<Json<Value>, AppError> {
    // Le nouveau mot de passe doit faire au moins 6 caractères
    if payload.new_password.len() < 6 {
        return Err(AppError::Validation(
            "Password must be at least 6 characters".to_string(),
        ));
    }

    // Recherche de l'utilisateur par username
    let user_opt = user::find_by_username(&state.db, &payload.username, &state.encryption_key).await?;
    let user = user_opt.ok_or(AppError::UserNotFound)?;

    // Vérification du code de récupération (comparaison par hash)
    let code_valid = user::verify_recovery_code(&state.db, user.id, &payload.recovery_code).await?;
    if !code_valid {
        return Err(AppError::InvalidCredentials);
    }

    // Hash du nouveau mot de passe et mise à jour en base
    let password_hash = bcrypt::hash(&payload.new_password, bcrypt::DEFAULT_COST)?;
    user::update_password(&state.db, user.id, &password_hash).await?;

    Ok(Json(json!({
        "message": "Password reset successfully"
    })))
}

/// Récupère le profil de l'utilisateur connecté.
///
/// Retourne les infos déchiffrées (username, email, profile_picture_url).
#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "Authentication",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Profil utilisateur", body = UserResponse),
        (status = 401, description = "Non authentifié", body = ErrorResponse),
    )
)]
pub async fn get_me(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;
    let user = user::find_by_id(&state.db, claims.sub, &state.encryption_key)
        .await?
        .ok_or(AppError::UserNotFound)?;

    let user_response: UserResponse = user.into();
    Ok(Json(json!({ "user": user_response })))
}

/// Met à jour la photo de profil de l'utilisateur connecté.
///
/// Accepte une URL de photo (issue de /upload) ou null pour supprimer la photo.
#[utoipa::path(
    put,
    path = "/auth/profile",
    tag = "Authentication",
    security(("bearer_auth" = [])),
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Profil mis à jour", body = UserResponse),
        (status = 401, description = "Non authentifié", body = ErrorResponse),
    )
)]
pub async fn update_profile(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    // Lecture du corps de la requête
    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|_| AppError::Validation("Invalid request body".to_string()))?;

    let payload: UpdateProfileRequest = serde_json::from_slice(&body)
        .map_err(|_| AppError::Validation("Invalid JSON body".to_string()))?;

    // Mettre à jour la photo de profil en base
    user::update_profile_picture(
        &state.db,
        claims.sub,
        payload.profile_picture_url.as_deref(),
    )
    .await?;

    // Retourner le profil mis à jour
    let user = user::find_by_id(&state.db, claims.sub, &state.encryption_key)
        .await?
        .ok_or(AppError::UserNotFound)?;

    let user_response: UserResponse = user.into();
    Ok(Json(json!({
        "message": "Profile updated",
        "user": user_response
    })))
}

/// Corps de la requête pour renouveler le token.
#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Renouvelle l'access token à l'aide d'un refresh token valide.
///
/// Le refresh token est à usage unique (rotation) : un nouveau refresh token
/// est retourné à chaque appel. L'ancien est invalidé.
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "Authentication",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "Tokens renouvelés", body = AuthResponse),
        (status = 401, description = "Refresh token invalide ou expiré", body = ErrorResponse),
    )
)]
pub async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<Value>, AppError> {
    // Vérifier et supprimer l'ancien refresh token (rotation)
    let user_id = verify_and_rotate_refresh_token(&state.db, &payload.refresh_token).await?;

    // Récupérer l'utilisateur pour inclure le username dans le JWT
    let user = user::find_by_id(&state.db, user_id, &state.encryption_key)
        .await?
        .ok_or(AppError::UserNotFound)?;

    // Générer un nouvel access token (1h) et un nouveau refresh token (7j)
    let token = create_token(user.id, &user.username, &state.jwt_secret)?;
    let new_refresh_token = create_refresh_token(&state.db, user.id).await?;

    let user_response: UserResponse = user.into();

    Ok(Json(json!({
        "message": "Token refreshed",
        "user": user_response,
        "token": token,
        "refresh_token": new_refresh_token
    })))
}

/// Déconnexion : supprime tous les refresh tokens de l'utilisateur.
///
/// Invalide toutes les sessions actives (logout global).
#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "Authentication",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Déconnexion réussie"),
        (status = 401, description = "Non authentifié", body = ErrorResponse),
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    req: Request,
) -> Result<Json<Value>, AppError> {
    let claims = extract_claims(&req, &state.jwt_secret)?;

    // Supprimer tous les refresh tokens de l'utilisateur
    delete_all_refresh_tokens(&state.db, claims.sub).await?;

    Ok(Json(json!({
        "message": "Logged out successfully"
    })))
}
