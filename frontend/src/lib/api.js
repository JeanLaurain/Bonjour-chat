/**
 * Client API — Toutes les requêtes HTTP vers le backend Bonjour.
 * Gère automatiquement le token JWT dans les headers et le renouvellement
 * via refresh token en cas d'expiration (401).
 */

function getToken() {
  return localStorage.getItem('token');
}

function getRefreshToken() {
  return localStorage.getItem('refresh_token');
}

function authHeaders(extra = {}) {
  const token = getToken();
  return {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...extra,
  };
}

/** Flag pour éviter plusieurs refresh simultanés */
let isRefreshing = false;
/** File d'attente des requêtes en attente du refresh */
let refreshQueue = [];

/**
 * Tente de renouveler l'access token via le refresh token.
 * En cas de succès, met à jour le localStorage et relance les requêtes en attente.
 */
async function tryRefreshToken() {
  const refreshToken = getRefreshToken();
  if (!refreshToken) throw new Error('No refresh token');

  const res = await fetch('/auth/refresh', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ refresh_token: refreshToken }),
  });

  if (!res.ok) {
    // Refresh token invalide ou expiré → déconnexion
    localStorage.removeItem('token');
    localStorage.removeItem('refresh_token');
    localStorage.removeItem('user');
    throw new Error('Refresh token expired');
  }

  const data = await res.json();
  // Stocker les nouveaux tokens
  localStorage.setItem('token', data.token);
  localStorage.setItem('refresh_token', data.refresh_token);
  if (data.user) localStorage.setItem('user', JSON.stringify(data.user));
  return data.token;
}

/**
 * Requête HTTP avec renouvellement automatique du token.
 * Si le serveur répond 401, tente un refresh puis rejoue la requête.
 */
async function request(url, options = {}) {
  let res = await fetch(url, options);

  // Si 401 et qu'on a un refresh token, tenter le renouvellement
  if (res.status === 401 && getRefreshToken()) {
    if (!isRefreshing) {
      isRefreshing = true;
      try {
        const newToken = await tryRefreshToken();
        // Débloquer toutes les requêtes en attente
        refreshQueue.forEach(cb => cb(newToken));
        refreshQueue = [];
      } catch {
        // Refresh échoué → déconnecter l'utilisateur
        refreshQueue.forEach(cb => cb(null));
        refreshQueue = [];
        window.location.reload();
        throw new Error('Session expired');
      } finally {
        isRefreshing = false;
      }
    } else {
      // Un refresh est déjà en cours, attendre qu'il finisse
      const newToken = await new Promise(resolve => refreshQueue.push(resolve));
      if (!newToken) throw new Error('Session expired');
    }

    // Rejouer la requête avec le nouveau token
    const newHeaders = { ...options.headers };
    if (newHeaders['Authorization'] || newHeaders['authorization']) {
      newHeaders['Authorization'] = `Bearer ${getToken()}`;
    }
    res = await fetch(url, { ...options, headers: newHeaders });
  }

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

export const resetPassword = (username, recoveryCode, newPassword) =>
  request('/auth/reset-password', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, recovery_code: recoveryCode, new_password: newPassword }),
  });

/** Récupère le profil de l'utilisateur connecté */
export const getMe = () =>
  request('/auth/me', { headers: authHeaders() });

/** Met à jour la photo de profil (url issue de /upload, ou null pour supprimer) */
export const updateProfile = (profilePictureUrl) =>
  request('/auth/profile', {
    method: 'PUT',
    headers: authHeaders(),
    body: JSON.stringify({ profile_picture_url: profilePictureUrl }),
  });

// ── Users ─────────────────────────────────────────────
export const searchUsers = (q) =>
  request(`/users/search?q=${encodeURIComponent(q)}`, { headers: authHeaders() });

// ── Messages (DM) ─────────────────────────────────────
export const sendMessage = (receiverId, content, messageType = 'text', imageUrl = null, originalFilename = null, replyToId = null) =>
  request('/messages', {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify({
      receiver_id: receiverId,
      content,
      message_type: messageType,
      image_url: imageUrl,
      original_filename: originalFilename,
      reply_to_id: replyToId,
    }),
  });

export const listConversations = () =>
  request('/conversations', { headers: authHeaders() });

export const getConversation = (userId, beforeId = null) => {
  let url = `/conversations/${userId}`;
  if (beforeId) url += `?before_id=${beforeId}`;
  return request(url, { headers: authHeaders() });
};

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

export const getGroupMessages = (id, beforeId = null) => {
  let url = `/groups/${id}/messages`;
  if (beforeId) url += `?before_id=${beforeId}`;
  return request(url, { headers: authHeaders() });
};

export const sendGroupMessage = (id, content, messageType = 'text', imageUrl = null, originalFilename = null, replyToId = null) =>
  request(`/groups/${id}/messages`, {
    method: 'POST',
    headers: authHeaders(),
    body: JSON.stringify({
      content,
      message_type: messageType,
      image_url: imageUrl,
      original_filename: originalFilename,
      reply_to_id: replyToId,
    }),
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

export const renameGroup = (id, name) =>
  request(`/groups/${id}`, {
    method: 'PUT',
    headers: authHeaders(),
    body: JSON.stringify({ name }),
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
