/**
 * Stores Svelte réactifs — état global de l'application Bonjour.
 */
import { writable, get } from 'svelte/store';

// ── Auth Store (persisté dans localStorage) ───────────
function createAuthStore() {
  const token = localStorage.getItem('token');
  const refreshToken = localStorage.getItem('refresh_token');
  const user = JSON.parse(localStorage.getItem('user') || 'null');
  const { subscribe, set } = writable({ token, refreshToken, user, isAuthenticated: !!token });

  return {
    subscribe,
    /** Stocke les tokens et l'utilisateur après login/register */
    login: (token, user, refreshToken) => {
      localStorage.setItem('token', token);
      if (refreshToken) localStorage.setItem('refresh_token', refreshToken);
      localStorage.setItem('user', JSON.stringify(user));
      set({ token, refreshToken, user, isAuthenticated: true });
    },
    /** Met à jour les données utilisateur (ex: après changement de photo de profil) */
    updateUser: (userData) => {
      const stored = JSON.parse(localStorage.getItem('user') || '{}');
      const merged = { ...stored, ...userData };
      localStorage.setItem('user', JSON.stringify(merged));
      const token = localStorage.getItem('token');
      const refreshToken = localStorage.getItem('refresh_token');
      set({ token, refreshToken, user: merged, isAuthenticated: !!token });
    },
    logout: () => {
      localStorage.removeItem('token');
      localStorage.removeItem('refresh_token');
      localStorage.removeItem('user');
      set({ token: null, refreshToken: null, user: null, isAuthenticated: false });
    },
  };
}
export const auth = createAuthStore();

// ── Conversation active ───────────────────────────────
export const currentConversation = writable(null);

// ── Statuts en ligne (user_id → boolean) ──────────────
export const onlineUsers = writable({});

// ── Compteurs non lus DM (user_id → count) ────────────
export const unreadCounts = writable({});

// ── Compteurs non lus groupes (group_id → count) ──────
export const groupUnreadCounts = writable({});

// ── Sidebar collapsed (persisté) ──────────────────────
function createSidebarStore() {
  const saved = localStorage.getItem('sidebarCollapsed') === 'true';
  const { subscribe, set, update } = writable(saved);
  return {
    subscribe,
    toggle: () => update(v => { const n = !v; localStorage.setItem('sidebarCollapsed', String(n)); return n; }),
    set: (v) => { localStorage.setItem('sidebarCollapsed', String(v)); set(v); },
  };
}
export const sidebarCollapsed = createSidebarStore();

// ── WebSocket ─────────────────────────────────────────
export function createWsConnection(token, onMessage) {
  let ws = null;
  let reconnectTimer = null;
  let intentionalClose = false;

  function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${window.location.host}/ws?token=${encodeURIComponent(token)}`);

    ws.onopen = () => {
      if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
    };
    ws.onmessage = (e) => {
      try { onMessage(JSON.parse(e.data)); } catch {}
    };
    ws.onclose = () => {
      if (!intentionalClose) reconnectTimer = setTimeout(connect, 3000);
    };
    ws.onerror = () => {};
  }

  connect();
  return {
    /** Envoie un message JSON via le WebSocket (utilisé pour le signaling WebRTC) */
    send: (data) => { if (ws && ws.readyState === WebSocket.OPEN) ws.send(typeof data === 'string' ? data : JSON.stringify(data)); },
    /** Vérifie si le WebSocket est connecté */
    get readyState() { return ws ? ws.readyState : WebSocket.CLOSED; },
    close: () => { intentionalClose = true; clearTimeout(reconnectTimer); ws?.close(); },
  };
}
