/**
 * Client API — Toutes les requêtes HTTP vers le backend Bonjour.
 * Gère automatiquement le token JWT dans les headers.
 */

function getToken() {
  return localStorage.getItem('token');
}

function authHeaders(extra = {}) {
  const token = getToken();
  return {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...extra,
  };
}

async function request(url, options = {}) {
  const res = await fetch(url, options);
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || body.message || `HTTP ${res.status}`);
  }
  return res.json();
}

// ── Auth ──────────────────────────────────────────────
export const register = (username, email, password) =>
  request('/auth/register', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, email, password }),
  });

export const login = (username, password) =>
  request('/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });

export const resetPassword = (username, email, newPassword) =>
  request('/auth/reset-password', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, email, new_password: newPassword }),
  });

// ── Users ─────────────────────────────────────────────
export const searchUsers = (q) =>
  request(`/users/search?q=${encodeURIComponent(q)}`, { headers: authHeaders() });

// ── Messages (DM) ─────────────────────────────────────
export const sendMessage = (receiverId, content, messageType = 'text', imageUrl = null) =>
  request('/messages', {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify({
      receiver_id: receiverId,
      content,
      message_type: messageType,
      image_url: imageUrl,
    }),
  });

export const listConversations = () =>
  request('/conversations', { headers: authHeaders() });

export const getConversation = (userId) =>
  request(`/conversations/${userId}`, { headers: authHeaders() });

export const markAsRead = (userId) =>
  request(`/conversations/${userId}/read`, {
    method: 'PUT',
    headers: authHeaders(),
  });

// ── Groups ────────────────────────────────────────────
export const createGroup = (name, memberIds) =>
  request('/groups', {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify({ name, member_ids: memberIds }),
  });

export const listGroups = () =>
  request('/groups', { headers: authHeaders() });

export const getGroup = (id) =>
  request(`/groups/${id}`, { headers: authHeaders() });

export const getGroupMessages = (id) =>
  request(`/groups/${id}/messages`, { headers: authHeaders() });

export const sendGroupMessage = (id, content, messageType = 'text', imageUrl = null) =>
  request(`/groups/${id}/messages`, {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify({ content, message_type: messageType, image_url: imageUrl }),
  });

export const addGroupMembers = (id, userIds) =>
  request(`/groups/${id}/members`, {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify({ user_ids: userIds }),
  });

export const removeGroupMember = (groupId, userId) =>
  request(`/groups/${groupId}/members/${userId}`, {
    method: 'DELETE',
    headers: authHeaders(),
  });

// ── Upload ────────────────────────────────────────────
export async function uploadImage(file) {
  const formData = new FormData();
  formData.append('file', file);
  const res = await fetch('/upload', {
    method: 'POST',
    headers: { Authorization: `Bearer ${getToken()}` },
    body: formData,
  });
  if (!res.ok) throw new Error('Upload failed');
  return res.json();
}

// ── Notifications ─────────────────────────────────────
export const getVapidKey = () =>
  request('/notifications/vapid-key', { headers: authHeaders() });

export const subscribePush = (subscription) =>
  request('/notifications/subscribe', {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify(subscription),
  });

export const unsubscribePush = (endpoint) =>
  request('/notifications/unsubscribe', {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify({ endpoint }),
  });
