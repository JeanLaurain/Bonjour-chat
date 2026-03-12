import './app.css';
import App from './App.svelte';

// Enregistrement du Service Worker pour la PWA
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch(() => {});
  });
}

const app = new App({ target: document.getElementById('app') });

export default app;
