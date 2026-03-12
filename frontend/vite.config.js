import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      '/auth': 'http://localhost:3000',
      '/messages': 'http://localhost:3000',
      '/conversations': 'http://localhost:3000',
      '/users': 'http://localhost:3000',
      '/upload': 'http://localhost:3000',
      '/uploads': 'http://localhost:3000',
      '/ws': { target: 'http://localhost:3000', ws: true },
      '/notifications': 'http://localhost:3000',
      '/groups': 'http://localhost:3000',
      '/swagger-ui': 'http://localhost:3000',
      '/api-docs': 'http://localhost:3000',
      '/health': 'http://localhost:3000',
    }
  }
});
