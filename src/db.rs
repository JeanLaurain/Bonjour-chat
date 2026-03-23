//! Utilitaires de base de données MongoDB.
//!
//! Fournit un générateur d'IDs séquentiels basé sur une collection `counters`.
//! Chaque collection (users, messages, etc.) a son propre compteur.

use bson::doc;
use mongodb::options::{FindOneAndUpdateOptions, ReturnDocument};
use mongodb::Database;

/// Génère le prochain ID séquentiel pour une collection donnée.
/// Utilise un pattern de compteur atomique dans la collection `counters`.
pub async fn next_id(db: &Database, collection_name: &str) -> Result<i32, mongodb::error::Error> {
    let counters = db.collection::<bson::Document>("counters");
    let options = FindOneAndUpdateOptions::builder()
        .upsert(true)
        .return_document(ReturnDocument::After)
        .build();

    let result = counters
        .find_one_and_update(
            doc! { "_id": collection_name },
            doc! { "$inc": { "seq": 1 } },
            options,
        )
        .await?;

    match result {
        Some(doc) => Ok(doc.get_i32("seq").unwrap_or(1)),
        None => Ok(1),
    }
}
