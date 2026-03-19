#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# test_api.sh — Tests automatisés de toutes les routes de l'API Bonjour
# 
# Usage: ./test_api.sh [BASE_URL]
# Défaut: http://localhost:3000
#
# Ce script teste: auth, messages, conversations, groupes,
# uploads, notifications, et la pagination.
# ═══════════════════════════════════════════════════════════════

set -e

BASE="${1:-http://localhost:3000}"
PASS=0
FAIL=0
TOTAL=0

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Utilisateurs de test (noms uniques par exécution)
RAND=$(date +%s)
USER_A="testuser_a_${RAND}"
USER_B="testuser_b_${RAND}"
USER_C="testuser_c_${RAND}"

# Fonction de test: effectue une requête et vérifie le code HTTP
assert_status() {
  local desc="$1"
  local expected="$2"
  local method="$3"
  local url="$4"
  shift 4
  local extra_args=("$@")

  TOTAL=$((TOTAL + 1))
  local response
  response=$(curl -s -o /dev/null -w "%{http_code}" -X "$method" "${BASE}${url}" "${extra_args[@]}" 2>/dev/null)

  if [ "$response" = "$expected" ]; then
    echo -e "  ${GREEN}✓${NC} ${desc} (HTTP ${response})"
    PASS=$((PASS + 1))
  else
    echo -e "  ${RED}✗${NC} ${desc} — attendu ${expected}, reçu ${response}"
    FAIL=$((FAIL + 1))
  fi
}

# Fonction qui retourne le body JSON
api_call() {
  local method="$1"
  local url="$2"
  shift 2
  curl -s -X "$method" "${BASE}${url}" "$@" 2>/dev/null
}

echo ""
echo -e "${YELLOW}═══ Tests de l'API Bonjour ═══${NC}"
echo -e "URL: ${BASE}"
echo ""

# ─────────────────────────────────────────────────────
echo -e "${YELLOW}▸ Health check${NC}"
assert_status "GET /health" "200" "GET" "/health"

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Auth — Inscription${NC}"

# Inscription user A
RESULT_A=$(api_call POST "/auth/register" -H "Content-Type: application/json" \
  -d "{\"username\":\"${USER_A}\",\"email\":\"${USER_A}@test.com\",\"password\":\"Password123!\"}")
TOKEN_A=$(echo "$RESULT_A" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
RECOVERY_A=$(echo "$RESULT_A" | grep -o '"recovery_codes":\[[^]]*\]' | head -1)

if [ -n "$TOKEN_A" ]; then
  echo -e "  ${GREEN}✓${NC} Register user A — token reçu"
  PASS=$((PASS + 1))
else
  echo -e "  ${RED}✗${NC} Register user A — pas de token"
  FAIL=$((FAIL + 1))
fi
TOTAL=$((TOTAL + 1))

# Inscription user B
RESULT_B=$(api_call POST "/auth/register" -H "Content-Type: application/json" \
  -d "{\"username\":\"${USER_B}\",\"email\":\"${USER_B}@test.com\",\"password\":\"Password456!\"}")
TOKEN_B=$(echo "$RESULT_B" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -n "$TOKEN_B" ]; then
  echo -e "  ${GREEN}✓${NC} Register user B — token reçu"
  PASS=$((PASS + 1))
else
  echo -e "  ${RED}✗${NC} Register user B — pas de token"
  FAIL=$((FAIL + 1))
fi
TOTAL=$((TOTAL + 1))

# Inscription user C
RESULT_C=$(api_call POST "/auth/register" -H "Content-Type: application/json" \
  -d "{\"username\":\"${USER_C}\",\"email\":\"${USER_C}@test.com\",\"password\":\"Password789!\"}")
TOKEN_C=$(echo "$RESULT_C" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -n "$TOKEN_C" ]; then
  echo -e "  ${GREEN}✓${NC} Register user C — token reçu"
  PASS=$((PASS + 1))
else
  echo -e "  ${RED}✗${NC} Register user C — pas de token"
  FAIL=$((FAIL + 1))
fi
TOTAL=$((TOTAL + 1))

# Doublon
assert_status "Register doublon" "400" "POST" "/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"${USER_A}\",\"email\":\"${USER_A}@test.com\",\"password\":\"Password123!\"}"

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Auth — Connexion${NC}"

# Login valide
RESULT_LOGIN=$(api_call POST "/auth/login" -H "Content-Type: application/json" \
  -d "{\"username\":\"${USER_A}\",\"password\":\"Password123!\"}")
TOKEN_A_LOGIN=$(echo "$RESULT_LOGIN" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -n "$TOKEN_A_LOGIN" ]; then
  echo -e "  ${GREEN}✓${NC} Login user A — token reçu"
  TOKEN_A="$TOKEN_A_LOGIN"
  PASS=$((PASS + 1))
else
  echo -e "  ${RED}✗${NC} Login user A — pas de token"
  FAIL=$((FAIL + 1))
fi
TOTAL=$((TOTAL + 1))

# Login invalide
assert_status "Login mauvais mot de passe" "401" "POST" "/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"${USER_A}\",\"password\":\"wrong\"}"

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Users — Recherche${NC}"

assert_status "Search users (auth)" "200" "GET" "/users/search?q=${USER_B}" \
  -H "Authorization: Bearer ${TOKEN_A}"

assert_status "Search users (sans auth)" "401" "GET" "/users/search?q=test"

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Messages — DM${NC}"

# Trouver l'ID de user B
USER_B_DATA=$(api_call GET "/users/search?q=${USER_B}" -H "Authorization: Bearer ${TOKEN_A}")
USER_B_ID=$(echo "$USER_B_DATA" | grep -o '"id":[0-9]*' | head -1 | cut -d: -f2)

if [ -z "$USER_B_ID" ]; then
  echo -e "  ${RED}✗${NC} Impossible de trouver user B — ID manquant"
  FAIL=$((FAIL + 1))
  TOTAL=$((TOTAL + 1))
else
  # Envoi de message texte
  assert_status "Send message (text)" "200" "POST" "/messages" \
    -H "Authorization: Bearer ${TOKEN_A}" \
    -H "Content-Type: application/json" \
    -d "{\"receiver_id\":${USER_B_ID},\"content\":\"Hello from test!\",\"message_type\":\"text\"}"

  # Envoi avec reply_to_id
  assert_status "Send message (reply)" "200" "POST" "/messages" \
    -H "Authorization: Bearer ${TOKEN_A}" \
    -H "Content-Type: application/json" \
    -d "{\"receiver_id\":${USER_B_ID},\"content\":\"Reply test\",\"message_type\":\"text\",\"reply_to_id\":null}"

  # Envoi sans auth
  assert_status "Send message (sans auth)" "401" "POST" "/messages" \
    -H "Content-Type: application/json" \
    -d "{\"receiver_id\":${USER_B_ID},\"content\":\"No auth\"}"

  # Conversation
  assert_status "Get conversation" "200" "GET" "/conversations/${USER_B_ID}" \
    -H "Authorization: Bearer ${TOKEN_A}"

  # Pagination (before_id)
  assert_status "Get conversation (paginated)" "200" "GET" "/conversations/${USER_B_ID}?before_id=9999" \
    -H "Authorization: Bearer ${TOKEN_A}"

  # Mark as read
  assert_status "Mark as read" "200" "PUT" "/conversations/${USER_B_ID}/read" \
    -H "Authorization: Bearer ${TOKEN_B}"
fi

# Liste des conversations
assert_status "List conversations" "200" "GET" "/conversations" \
  -H "Authorization: Bearer ${TOKEN_A}"

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Groups${NC}"

# Trouver l'ID de user C
USER_C_DATA=$(api_call GET "/users/search?q=${USER_C}" -H "Authorization: Bearer ${TOKEN_A}")
USER_C_ID=$(echo "$USER_C_DATA" | grep -o '"id":[0-9]*' | head -1 | cut -d: -f2)

# Créer un groupe
GROUP_RESULT=$(api_call POST "/groups" \
  -H "Authorization: Bearer ${TOKEN_A}" \
  -H "Content-Type: application/json" \
  -d "{\"name\":\"Test Group ${RAND}\",\"member_ids\":[${USER_B_ID}]}")
GROUP_ID=$(echo "$GROUP_RESULT" | grep -o '"id":[0-9]*' | head -1 | cut -d: -f2)

if [ -n "$GROUP_ID" ]; then
  echo -e "  ${GREEN}✓${NC} Create group — id=${GROUP_ID}"
  PASS=$((PASS + 1))
else
  echo -e "  ${RED}✗${NC} Create group — pas d'ID"
  FAIL=$((FAIL + 1))
fi
TOTAL=$((TOTAL + 1))

# Liste des groupes
assert_status "List groups" "200" "GET" "/groups" \
  -H "Authorization: Bearer ${TOKEN_A}"

if [ -n "$GROUP_ID" ]; then
  # Détail du groupe
  assert_status "Get group" "200" "GET" "/groups/${GROUP_ID}" \
    -H "Authorization: Bearer ${TOKEN_A}"

  # Envoyer un message dans le groupe
  assert_status "Send group message" "200" "POST" "/groups/${GROUP_ID}/messages" \
    -H "Authorization: Bearer ${TOKEN_A}" \
    -H "Content-Type: application/json" \
    -d "{\"content\":\"Hello group!\",\"message_type\":\"text\"}"

  # Envoyer avec original_filename et reply_to_id
  assert_status "Send group msg (with extras)" "200" "POST" "/groups/${GROUP_ID}/messages" \
    -H "Authorization: Bearer ${TOKEN_A}" \
    -H "Content-Type: application/json" \
    -d "{\"content\":\"/uploads/test.png\",\"message_type\":\"image\",\"image_url\":\"/uploads/test.png\",\"original_filename\":\"photo.png\"}"

  # Messages du groupe
  assert_status "Get group messages" "200" "GET" "/groups/${GROUP_ID}/messages" \
    -H "Authorization: Bearer ${TOKEN_A}"

  # Pagination des messages du groupe
  assert_status "Get group messages (paginated)" "200" "GET" "/groups/${GROUP_ID}/messages?before_id=9999" \
    -H "Authorization: Bearer ${TOKEN_A}"

  # Renommer le groupe
  assert_status "Rename group" "200" "PUT" "/groups/${GROUP_ID}" \
    -H "Authorization: Bearer ${TOKEN_A}" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"Renamed Group ${RAND}\"}"

  # Ajouter un membre
  if [ -n "$USER_C_ID" ]; then
    assert_status "Add group member" "200" "POST" "/groups/${GROUP_ID}/members" \
      -H "Authorization: Bearer ${TOKEN_A}" \
      -H "Content-Type: application/json" \
      -d "{\"user_ids\":[${USER_C_ID}]}"

    # Supprimer un membre
    assert_status "Remove group member" "200" "DELETE" "/groups/${GROUP_ID}/members/${USER_C_ID}" \
      -H "Authorization: Bearer ${TOKEN_A}"

    # Supprimer un membre inexistant (devrait être 404 ou 400)
    assert_status "Remove non-member (expect 404/400)" "404" "DELETE" "/groups/${GROUP_ID}/members/99999" \
      -H "Authorization: Bearer ${TOKEN_A}"
  fi
fi

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Uploads${NC}"

# Upload sans auth
assert_status "Upload sans auth" "401" "POST" "/upload"

# Upload avec auth (fichier texte fictif)
echo "test file content" > /tmp/test_upload.txt
UPLOAD_RESULT=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${BASE}/upload" \
  -H "Authorization: Bearer ${TOKEN_A}" \
  -F "file=@/tmp/test_upload.txt" 2>/dev/null)
TOTAL=$((TOTAL + 1))
if [ "$UPLOAD_RESULT" = "200" ]; then
  echo -e "  ${GREEN}✓${NC} Upload file — HTTP 200"
  PASS=$((PASS + 1))
else
  echo -e "  ${RED}✗${NC} Upload file — attendu 200, reçu ${UPLOAD_RESULT}"
  FAIL=$((FAIL + 1))
fi
rm -f /tmp/test_upload.txt

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Notifications${NC}"

assert_status "Get VAPID key" "200" "GET" "/notifications/vapid-key" \
  -H "Authorization: Bearer ${TOKEN_A}"

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Auth — Reset password${NC}"

# Essai avec un code invalide
assert_status "Reset password (bad code)" "401" "POST" "/auth/reset-password" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"${USER_A}\",\"recovery_code\":\"WRONGCODE\",\"new_password\":\"NewPass123!\"}"

# ─────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}▸ Swagger UI${NC}"
assert_status "Swagger UI" "200" "GET" "/swagger-ui/"

# ═══════════════════════════════════════════════════════
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "  Total: ${TOTAL} | ${GREEN}Passés: ${PASS}${NC} | ${RED}Échoués: ${FAIL}${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
