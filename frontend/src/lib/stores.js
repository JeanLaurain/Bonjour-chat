/**
 * Stores Svelte réactifs — état global de l'application Bonjour.
 */
import { writable, get } from 'svelte/store';

// ── Auth Store (persisté dans localStorage) ───────────
function createAuthStore() {
  const token = localStorage.getItem('token');
  const user = JSON.parse(localStorage.getItem('user') || 'null');
  const { subscribe, set } = writable({ token, user, isAuthenticated: !!token });

  return {
    subscribe,
    login: (token, user) => {
      localStorage.setItem('token', token);
      localStorage.setItem('user', JSON.stringify(user));
      set({ token, user, isAuthenticated: true });
    },
    logout: () => {
      localStorage.removeItem('token');
      localStorage.removeItem('user');
      set({ token: null, user: null, isAuthenticated: false });
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
    close: () => { intentionalClose = true; clearTimeout(reconnectTimer); ws?.close(); },
  };
}
