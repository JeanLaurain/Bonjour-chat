<!--
  NewConversation — Modale pour rechercher un utilisateur et démarrer une conversation DM.
-->
<script>
  import { createEventDispatcher } from 'svelte';
  import * as api from '../lib/api.js';

  const dispatch = createEventDispatcher();
  let query = '';
  let results = [];
  let loading = false;
  let searchTimeout;

  function handleSearch() {
    clearTimeout(searchTimeout);
    if (query.trim().length < 2) { results = []; return; }
    searchTimeout = setTimeout(async () => {
      loading = true;
      try {
        const data = await api.searchUsers(query.trim());
        results = data.users || [];
      } catch { results = []; }
      loading = false;
    }, 300);
  }

  function selectUser(user) {
    dispatch('select', { user });
  }

  function close() { dispatch('close'); }

  function handleKeydown(e) {
    if (e.key === 'Escape') close();
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- Fond sombre de la modale -->
<div class="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 animate-fade-in" on:click|self={close}>
  <div class="bg-slate-800 rounded-2xl shadow-2xl border border-slate-700/50 w-full max-w-md animate-slide-up">
    <!-- En-tête -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-slate-700/50">
      <h3 class="text-lg font-semibold text-white">Nouvelle conversation</h3>
      <button on:click={close} class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-700 transition-colors">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
      </button>
    </div>

    <!-- Champ de recherche -->
    <div class="px-6 py-4">
      <div class="relative">
        <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>
        <input
          type="text"
          bind:value={query}
          on:input={handleSearch}
          placeholder="Rechercher un utilisateur..."
          autofocus
          class="w-full pl-10 pr-4 py-2.5 bg-slate-900/50 border border-slate-600/50 rounded-xl text-sm text-white
                 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
        />
      </div>
    </div>

    <!-- Résultats de recherche -->
    <div class="max-h-64 overflow-y-auto px-3 pb-4">
      {#if loading}
        <div class="flex justify-center py-8">
          <svg class="w-6 h-6 text-primary-400 animate-spin" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"/><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/></svg>
        </div>
      {:else if results.length > 0}
        {#each results as user}
          <button
            on:click={() => selectUser(user)}
            class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-slate-700/50 transition-colors text-left"
          >
            <div class="w-10 h-10 rounded-full bg-primary-500 flex items-center justify-center text-white text-sm font-semibold flex-shrink-0">
              {(user.username || '?')[0].toUpperCase()}
            </div>
            <div class="min-w-0">
              <p class="text-sm font-medium text-white truncate">{user.username}</p>
              {#if user.email}
                <p class="text-xs text-slate-400 truncate">{user.email}</p>
              {/if}
            </div>
          </button>
        {/each}
      {:else if query.trim().length >= 2}
        <p class="text-center text-slate-500 text-sm py-8">Aucun utilisateur trouvé</p>
      {:else}
        <p class="text-center text-slate-500 text-sm py-8">Tapez au moins 2 caractères pour rechercher</p>
      {/if}
    </div>
  </div>
</div>
