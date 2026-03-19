//! Handler d'upload de fichiers.
//!
//! Accepte tout type de fichier via multipart/form-data,
//! le sauvegarde avec un nom UUID unique dans /app/uploads/,
//! et retourne l'URL relative + le nom original du fichier.

use axum::{extract::Multipart, Json};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

use crate::errors::AppError;

/// Upload d'un fichier via multipart/form-data.
///
/// Le champ du formulaire doit s'appeler "file".
/// Aucune limite de taille ni de type de fichier.
/// Retourne l'URL relative, le nom UUID et le nom original du fichier.
#[utoipa::path(
    post,
    path = "/upload",
    tag = "Uploads",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Fichier uploadé avec succès"),
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
        // Récupérer le nom original du fichier
        let original_name = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "file".to_string());

        // Extraire l'extension pour le nom UUID
        let extension = Path::new(&original_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "bin".to_string());

        // Lire le contenu complet du fichier
        let data = field
            .bytes()
            .await
            .map_err(|_| AppError::Validation("Erreur de lecture du fichier".to_string()))?;

        // Nom unique avec UUID pour éviter les conflits et masquer le nom original
        let unique_name = format!("{}.{}", Uuid::new_v4(), extension);
        let file_path = format!("uploads/{}", unique_name);

        // Créer le dossier uploads si nécessaire
        tokio::fs::create_dir_all("uploads")
            .await
            .map_err(|_| AppError::Validation("Erreur serveur (filesystem)".to_string()))?;

        // Sauvegarder le fichier sur le disque
        tokio::fs::write(&file_path, &data)
            .await
            .map_err(|_| AppError::Validation("Erreur d'écriture du fichier".to_string()))?;

        let url = format!("/uploads/{}", unique_name);
        tracing::info!("Fichier uploadé : {} ({} bytes, original: {})", url, data.len(), original_name);

        return Ok(Json(json!({
            "url": url,
            "filename": unique_name,
            "original_filename": original_name,
            "size": data.len()
        })));
    }

    Err(AppError::Validation(
        "Aucun fichier trouvé dans la requête".to_string(),
    ))
}
