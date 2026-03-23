-- Migration 001 : Schéma initial
-- Tables : users, messages, push_subscriptions

CREATE TABLE IF NOT EXISTS users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(500) NOT NULL,
    email VARCHAR(500) NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    username_hash VARCHAR(64) DEFAULT NULL,
    email_hash VARCHAR(64) DEFAULT NULL,
    recovery_code_hash VARCHAR(64) DEFAULT NULL,
    profile_picture_url VARCHAR(500) DEFAULT NULL,
    last_seen DATETIME DEFAULT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_username_hash (username_hash),
    UNIQUE KEY uq_email_hash (email_hash)
);

CREATE TABLE IF NOT EXISTS messages (
    id INT AUTO_INCREMENT PRIMARY KEY,
    sender_id INT NOT NULL,
    receiver_id INT NOT NULL,
    content TEXT NOT NULL,
    message_type VARCHAR(10) DEFAULT 'text',
    image_url VARCHAR(500) DEFAULT NULL,
    original_filename VARCHAR(500) DEFAULT NULL,
    reply_to_id INT DEFAULT NULL,
    is_read BOOLEAN DEFAULT FALSE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id INT NOT NULL,
    endpoint VARCHAR(500) NOT NULL,
    p256dh VARCHAR(200) NOT NULL,
    auth VARCHAR(100) NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE KEY uq_user_endpoint (user_id, endpoint)
);

CREATE INDEX idx_messages_sender ON messages(sender_id);
CREATE INDEX idx_messages_receiver ON messages(receiver_id);
CREATE INDEX idx_messages_conversation ON messages(sender_id, receiver_id, created_at);
CREATE INDEX idx_messages_unread ON messages(receiver_id, sender_id, is_read);
CREATE INDEX idx_push_subs_user ON push_subscriptions(user_id);
