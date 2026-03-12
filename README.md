# Bonjour - Chat API

REST API de messagerie directe avec authentification JWT, construite avec Rust (Axum) et MySQL.

## Lancer avec Docker

```bash
cd C:\Repos\Bonjour
docker-compose up --build
```

**Services disponibles :**
- API : http://localhost:3000
- Health check : http://localhost:3000/health
- MySQL : localhost:3306

## Endpoints API

### Authentification

**Inscription**
```bash
curl -X POST http://localhost:3000/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "email": "alice@example.com", "password": "secret123"}'
```

**Connexion**
```bash
curl -X POST http://localhost:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "password": "secret123"}'
```

### Messages (JWT requis)

**Envoyer un message**
```bash
curl -X POST http://localhost:3000/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <TOKEN>" \
  -d '{"receiver_id": 2, "content": "Bonjour!"}'
```

**Voir une conversation**
```bash
curl http://localhost:3000/messages/2 \
  -H "Authorization: Bearer <TOKEN>"
```

**Lister les conversations**
```bash
curl http://localhost:3000/conversations \
  -H "Authorization: Bearer <TOKEN>"
```

## Stack technique

- **Backend** : Rust + Axum
- **Base de données** : MySQL 8.0
- **Auth** : JWT (jsonwebtoken) + bcrypt
- **ORM** : SQLx (async)
- **Conteneurisation** : Docker + Docker Compose
