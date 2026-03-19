/**
 * Service Worker — Cache-first pour les assets statiques,
 * network-first pour l'API. Gère les notifications push.
 */
const CACHE_NAME = 'bonjour-v3';
const PRECACHE = ['/', '/index.html'];

self.addEventListener('install', (e) => {
  e.waitUntil(
    caches.open(CACHE_NAME).then(c => c.addAll(PRECACHE)).then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then(keys =>
      Promise.all(keys.filter(k => k !== CACHE_NAME).map(k => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url);
  // API, WebSocket et uploads : toujours réseau
  if (url.pathname.startsWith('/auth') || url.pathname.startsWith('/messages') ||
      url.pathname.startsWith('/conversations') || url.pathname.startsWith('/users') ||
      url.pathname.startsWith('/upload') || url.pathname.startsWith('/groups') ||
      url.pathname.startsWith('/notifications') || url.pathname.startsWith('/ws') ||
      url.pathname.startsWith('/swagger-ui') || url.pathname.startsWith('/api-docs') ||
      url.pathname.startsWith('/uploads')) {
    return;
  }
  // Cache-first pour les assets statiques
  e.respondWith(
    caches.match(e.request).then(r => r || fetch(e.request).then(res => {
      if (res.ok && e.request.method === 'GET') {
        const clone = res.clone();
        caches.open(CACHE_NAME).then(c => c.put(e.request, clone));
      }
      return res;
    })).catch(() => caches.match('/index.html'))
  );
});

// Notifications push — parse le payload pour afficher un titre et un corps personnalisés
self.addEventListener('push', (e) => {
  let title = 'Bonjour';
  let body = 'Vous avez un nouveau message';
  let data = {};

  // Tenter de parser le payload JSON envoyé par le serveur
  if (e.data) {
    try {
      const payload = e.data.json();
      title = payload.title || title;
      body = payload.body || body;
      data = payload.data || {};
    } catch {
      // Si ce n'est pas du JSON, utiliser le texte brut
      body = e.data.text() || body;
    }
  }

  const options = {
    body,
    icon: '/icon-192.svg',
    badge: '/icon-192.svg',
    vibrate: [200, 100, 200],
    data,
    // Permettre au navigateur de regrouper les notifications similaires
    tag: data.conversation_id || 'bonjour-default',
    renotify: true,
    // Actions rapides sur la notification
    actions: [
      { action: 'open', title: 'Ouvrir' },
      { action: 'dismiss', title: 'Ignorer' }
    ]
  };

  e.waitUntil(self.registration.showNotification(title, options));
});

// Clic sur une notification — ouvrir l'app ou focus sur un onglet existant
self.addEventListener('notificationclick', (e) => {
  e.notification.close();

  if (e.action === 'dismiss') return;

  e.waitUntil(
    clients.matchAll({ type: 'window', includeUncontrolled: true }).then(windowClients => {
      // Focus sur un onglet existant si possible
      for (const client of windowClients) {
        if (client.url.includes(self.location.origin) && 'focus' in client) {
          return client.focus();
        }
      }
      // Sinon, ouvrir un nouvel onglet
      return clients.openWindow('/');
    })
  );
});
