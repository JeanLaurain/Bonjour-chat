#!/bin/bash
# =============================================================================
# Script de build des images Docker pour l'application Bonjour Chat
# Usage : ./build.sh [--no-cache]
# =============================================================================

set -e

NO_CACHE=""
if [ "$1" = "--no-cache" ]; then
  NO_CACHE="--no-cache"
  echo "🔄 Build sans cache activé"
fi

echo "============================================"
echo "  🏗️  Build des images Docker - Bonjour Chat"
echo "============================================"

# Créer les dossiers de données s'ils n'existent pas
echo ""
echo "📁 Création des dossiers de données..."
mkdir -p ./data/mysql
mkdir -p ./data/uploads

# Build de l'image backend (Rust/Axum)
echo ""
echo "🦀 Build de l'image backend (bonjour-app)..."
docker build $NO_CACHE -t bonjour-app -f Dockerfile .
echo "✅ Image backend construite avec succès"

# Build de l'image frontend (Svelte/Nginx)
echo ""
echo "🎨 Build de l'image frontend (bonjour-frontend)..."
docker build $NO_CACHE -t bonjour-frontend -f frontend/Dockerfile ./frontend
echo "✅ Image frontend construite avec succès"

echo ""
echo "============================================"
echo "  ✅ Toutes les images sont prêtes !"
echo "============================================"
echo ""
echo "Pour lancer l'application :"
echo "  docker compose up -d"
echo ""
echo "Pour voir les logs :"
echo "  docker compose logs -f"
echo ""
