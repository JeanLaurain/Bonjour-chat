#!/usr/bin/env bash
# =============================================================
# Bonjour Chat — Script de migration MySQL
#
# Exécute les scripts de migration sur une base de données
# existante. Chaque migration utilise CREATE TABLE IF NOT EXISTS
# ou des vérifications, donc elles sont idempotentes.
#
# Usage :
#   ./db/migrate.sh                    # Toutes les migrations
#   ./db/migrate.sh 003                # Une migration spécifique
#
# Variables d'environnement (ou depuis .env) :
#   MYSQL_ROOT_PASSWORD, MYSQL_DATABASE, MYSQL_CONTAINER
# =============================================================

set -euo pipefail

# Charger le .env s'il existe
if [ -f .env ]; then
  export $(grep -v '^#' .env | xargs)
fi

CONTAINER="${MYSQL_CONTAINER:-chat_mysql}"
DATABASE="${MYSQL_DATABASE:-chat_db}"
PASSWORD="${MYSQL_ROOT_PASSWORD:-rootpassword}"
MIGRATION_DIR="$(dirname "$0")/migrations"

echo "╔══════════════════════════════════════════╗"
echo "║   Bonjour Chat — Migration de la BDD     ║"
echo "╚══════════════════════════════════════════╝"
echo ""
echo "Container : $CONTAINER"
echo "Database  : $DATABASE"
echo ""

# Si un numéro de migration est fourni, n'exécuter que celle-là
if [ -n "${1:-}" ]; then
  FILE=$(ls "$MIGRATION_DIR"/${1}*.sql 2>/dev/null | head -1)
  if [ -z "$FILE" ]; then
    echo "❌ Migration $1 introuvable dans $MIGRATION_DIR"
    exit 1
  fi
  echo "▶ Exécution de $(basename "$FILE") ..."
  docker exec -i "$CONTAINER" mysql -u root -p"$PASSWORD" "$DATABASE" < "$FILE"
  echo "✅ $(basename "$FILE") terminé."
  exit 0
fi

# Exécuter toutes les migrations dans l'ordre
for FILE in "$MIGRATION_DIR"/*.sql; do
  echo "▶ Exécution de $(basename "$FILE") ..."
  docker exec -i "$CONTAINER" mysql -u root -p"$PASSWORD" "$DATABASE" < "$FILE"
  echo "  ✅ OK"
done

echo ""
echo "🎉 Toutes les migrations ont été appliquées avec succès."
