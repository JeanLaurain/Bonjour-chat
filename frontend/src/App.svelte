<script>
  import { auth } from './lib/stores.js';
  import Auth from './components/Auth.svelte';
  import ChatLayout from './components/ChatLayout.svelte';
</script>

{#if $auth.isAuthenticated}
  <ChatLayout />
{:else}
  <Auth />
{/if}
