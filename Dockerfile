FROM rust:latest AS builder

WORKDIR /app

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock* ./

# Create a dummy main.rs to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Copy actual source code
COPY src ./src

# Force recompilation of the real source (touch invalidates the cached dummy)
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/bonjour ./bonjour

# Copier la clé VAPID pour les notifications push
COPY vapid_private.pem /app/vapid_private.pem

# Créer le dossier d'uploads pour les images
RUN mkdir -p /app/uploads

EXPOSE 3000

CMD ["./bonjour"]
