//! Handler d'upload de fichiers (images).
//!
//! Accepte les images via multipart/form-data, les sauvegarde
//! avec un nom UUID unique dans le dossier /app/uploads/,
//! et retourne l'URL relative pour l'intégrer dans un message.

use axum::{extract::Multipart, Json};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

use crate::errors::AppError;

/// Taille maximale d'upload : 5 Mo
const MAX_FILE_SIZE: usize = 5 * 1024 * 1024;

/// Extensions de fichiers image autorisées
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp"];

/// Upload d'une image via multipart/form-data.
///
/// Le champ du formulaire doit s'appeler "file".
/// Retourne l'URL relative du fichier sauvegardé.
#[utoipa::path(
    post,
    path = "/upload",
    tag = "Uploads",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Image uploadée avec succès"),
        (status = 400, description = "Erreur de validation", body = crate::errors::ErrorResponse),
    )
)]
pub async fn upload_image(
    mut multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Validation("Données multipart invalides".to_string()))?
    {
        let file_name = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_default();

        // Vérifier l'extension
        let extension = Path::new(&file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
            return Err(AppError::Validation(format!(
                "Type de fichier non supporté. Autorisés : {:?}",
                ALLOWED_EXTENSIONS
            )));
        }

        // Lire le contenu
        let data = field
            .bytes()
            .await
            .map_err(|_| AppError::Validation("Erreur de lecture du fichier".to_string()))?;

        // Vérifier la taille
        if data.len() > MAX_FILE_SIZE {
            return Err(AppError::Validation(
                "Fichier trop volumineux (max 5 Mo)".to_string(),
            ));
        }

        // Nom unique avec UUID
        let unique_name = format!("{}.{}", Uuid::new_v4(), extension);
        let file_path = format!("uploads/{}", unique_name);

        // Créer le dossier si nécessaire
        tokio::fs::create_dir_all("uploads")
            .await
            .map_err(|_| AppError::Validation("Erreur serveur (filesystem)".to_string()))?;

        // Sauvegarder le fichier
        tokio::fs::write(&file_path, &data)
            .await
            .map_err(|_| AppError::Validation("Erreur d'écriture du fichier".to_string()))?;

        let url = format!("/uploads/{}", unique_name);
        tracing::info!("Image uploadée : {} ({} bytes)", url, data.len());

        return Ok(Json(json!({
            "url": url,
            "filename": unique_name,
            "size": data.len()
        })));
    }

    Err(AppError::Validation(
        "Aucun fichier trouvé dans la requête".to_string(),
    ))
}
