/**
 * Gestion des notifications push et locales.
 */
import * as api from './api.js';

/** Demande la permission de notifications au navigateur */
export async function requestNotificationPermission() {
  if (!('Notification' in window)) return 'denied';
  if (Notification.permission === 'granted') return 'granted';
  return Notification.requestPermission();
}

/** Affiche une notification locale si l'onglet n'est pas actif */
export function showLocalNotification(title, body) {
  if (document.hasFocus()) return;
  if (Notification.permission === 'granted') {
    new Notification(title, { body, icon: '/icon-192.svg', badge: '/icon-192.svg' });
  }
}

/** Configure l'abonnement push via VAPID */
export async function setupPushSubscription() {
  if (!('serviceWorker' in navigator) || !('PushManager' in window)) return;
  try {
    const data = await api.getVapidKey();
    if (!data.vapid_key) return;
    const reg = await navigator.serviceWorker.ready;
    const existing = await reg.pushManager.getSubscription();
    if (existing) return;

    const sub = await reg.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToUint8Array(data.vapid_key),
    });
    const json = sub.toJSON();
    await api.subscribePush({
      endpoint: json.endpoint,
      p256dh: json.keys.p256dh,
      auth: json.keys.auth,
    });
  } catch (e) {
    console.warn('Push setup failed:', e);
  }
}

function urlBase64ToUint8Array(base64String) {
  const padding = '='.repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
  const raw = atob(base64);
  return Uint8Array.from([...raw].map(c => c.charCodeAt(0)));
}
