//! # Bonjour — API de messagerie directe
//!
//! Serveur REST construit avec Axum, JWT et MySQL.
//! Swagger UI disponible sur `/swagger-ui`.

mod config;
mod crypto;
mod errors;
mod handlers;
mod middleware;
mod models;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use sqlx::mysql::MySqlPoolOptions;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::AppState;

/// Définition OpenAPI regroupant tous les endpoints et schémas.
/// C'est ici que utoipa génère la spec JSON servie par Swagger UI.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Bonjour Chat API",
        version = "0.1.0",
        description = "API REST de messagerie directe avec authentification JWT"
    ),
    paths(
        handlers::auth::register,
        handlers::auth::login,
        handlers::auth::reset_password,
        handlers::messages::send_message,
        handlers::messages::get_conversation,
        handlers::messages::list_conversations,
        handlers::messages::mark_as_read,
        handlers::users::search_users,
        handlers::uploads::upload_image,
    ),
    components(schemas(
        models::user::CreateUser,
        models::user::LoginUser,
        models::user::ResetPasswordRequest,
        models::user::UserResponse,
        models::message::Message,
        models::message::CreateMessage,
        models::message::ConversationPreview,
        errors::ErrorResponse,
        errors::AuthResponse,
        errors::UserInfo,
        errors::SendMessageResponse,
        errors::ConversationResponse,
        errors::ConversationsListResponse,
        errors::UsersSearchResponse,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "Authentication", description = "Inscription et connexion"),
        (name = "Messages", description = "Envoi et consultation de messages"),
        (name = "Users", description = "Recherche d'utilisateurs"),
        (name = "Uploads", description = "Upload d'images"),
        (name = "Notifications", description = "Notifications push"),
        (name = "WebSocket", description = "Connexion temps réel")
    )
)]
struct ApiDoc;

/// Ajoute le schéma de sécurité Bearer JWT à la spec OpenAPI
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialisation du logger (filtré via la variable RUST_LOG)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Lecture des variables d'environnement obligatoires
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret =
        std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    // Clés VAPID pour les notifications push
    let vapid_public_key =
        std::env::var("VAPID_PUBLIC_KEY").unwrap_or_default();
    let vapid_private_key_file =
        std::env::var("VAPID_PRIVATE_KEY_FILE").unwrap_or_else(|_| "vapid_private.pem".to_string());
    let vapid_private_pem = std::fs::read_to_string(&vapid_private_key_file)
        .unwrap_or_else(|e| {
            tracing::warn!("Could not read VAPID private key from {}: {}. Push notifications disabled.", vapid_private_key_file, e);
            String::new()
        });

    // Clé de chiffrement AES-256 (64 caractères hex = 32 octets)
    let encryption_key_hex = std::env::var("ENCRYPTION_KEY")
        .unwrap_or_else(|_| "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string());
    let encryption_key_bytes = hex::decode(&encryption_key_hex)
        .expect("ENCRYPTION_KEY must be a valid 64-character hex string");
    let mut encryption_key = [0u8; 32];
    encryption_key.copy_from_slice(&encryption_key_bytes);

    // Création du pool de connexions MySQL (max 10 connexions simultanées)
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("Failed to connect to MySQL");

    tracing::info!("Connected to MySQL");

    // État partagé entre tous les handlers (pool DB + clé JWT + WebSocket + VAPID)
    let ws_connections = Arc::new(RwLock::new(HashMap::new()));
    let state = AppState {
        db: pool,
        jwt_secret,
        ws_connections,
        vapid_public_key,
        vapid_private_pem,
        encryption_key,
    };

    // Construction du routeur avec tous les endpoints
    let app = Router::new()
        // --- Endpoints publics ---
        .route("/health", get(health))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/reset-password", post(handlers::auth::reset_password))
        // --- Endpoints protégés (JWT requis dans le header Authorization) ---
        .route("/messages", post(handlers::messages::send_message))
        .route("/conversations", get(handlers::messages::list_conversations))
        .route("/conversations/:user_id", get(handlers::messages::get_conversation))
        .route("/conversations/:user_id/read", put(handlers::messages::mark_as_read))
        .route("/users/search", get(handlers::users::search_users))
        .route("/upload", post(handlers::uploads::upload_image))
        // --- Notifications push ---
        .route("/notifications/vapid-key", get(handlers::notifications::get_vapid_key))
        .route("/notifications/subscribe", post(handlers::notifications::subscribe_push))
        .route("/notifications/unsubscribe", post(handlers::notifications::unsubscribe_push))
        // --- Groupes de conversation ---
        .route("/groups", get(handlers::groups::list_groups).post(handlers::groups::create_group))
        .route("/groups/:id", get(handlers::groups::get_group))
        .route("/groups/:id/messages", get(handlers::groups::get_group_messages).post(handlers::groups::send_group_message))
        .route("/groups/:id/members", post(handlers::groups::add_members))
        .route("/groups/:id/members/:user_id", delete(handlers::groups::remove_member))
        // --- WebSocket temps réel ---
        .route("/ws", get(handlers::ws::ws_handler))
        // --- Fichiers statiques (images uploadées) ---
        .nest_service("/uploads", ServeDir::new("uploads"))
        // --- Swagger UI : interface interactive de documentation ---
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // --- Middlewares globaux ---
        .layer(CorsLayer::permissive())  // Autorise toutes les origines (dev)
        .layer(TraceLayer::new_for_http()) // Log chaque requête HTTP
        .with_state(state);

    // Écoute sur toutes les interfaces, port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    tracing::info!("Server running on http://0.0.0.0:3000");
    tracing::info!("Swagger UI: http://0.0.0.0:3000/swagger-ui");
    axum::serve(listener, app).await.unwrap();
}

/// Endpoint de vérification de santé — renvoie "OK" si le serveur tourne
async fn health() -> &'static str {
    "OK"
}
