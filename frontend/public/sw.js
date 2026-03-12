/**
 * Service Worker — Cache-first pour les assets statiques,
 * network-first pour l'API. Gère les notifications push.
 */
const CACHE_NAME = 'bonjour-v2';
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
  // API et WebSocket : toujours réseau
  if (url.pathname.startsWith('/auth') || url.pathname.startsWith('/messages') ||
      url.pathname.startsWith('/conversations') || url.pathname.startsWith('/users') ||
      url.pathname.startsWith('/upload') || url.pathname.startsWith('/groups') ||
      url.pathname.startsWith('/notifications') || url.pathname.startsWith('/ws') ||
      url.pathname.startsWith('/swagger-ui') || url.pathname.startsWith('/api-docs')) {
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

// Notifications push
self.addEventListener('push', (e) => {
  const options = {
    body: 'Vous avez un nouveau message',
    icon: '/icon-192.svg',
    badge: '/icon-192.svg',
    vibrate: [100, 50, 100],
  };
  e.waitUntil(self.registration.showNotification('Bonjour', options));
});

self.addEventListener('notificationclick', (e) => {
  e.notification.close();
  e.waitUntil(clients.openWindow('/'));
});
