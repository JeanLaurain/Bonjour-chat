<!--
  CreateGroup — Modale pour créer un groupe de conversation.
  Permet de chercher des utilisateurs et de les ajouter comme membres.
-->
<script>
  import { createEventDispatcher } from 'svelte';
  import * as api from '../lib/api.js';

  const dispatch = createEventDispatcher();
  let groupName = '';
  let query = '';
  let searchResults = [];
  let selectedMembers = [];
  let loading = false;
  let creating = false;
  let error = '';
  let searchTimeout;

  function handleSearch() {
    clearTimeout(searchTimeout);
    if (query.trim().length < 2) { searchResults = []; return; }
    searchTimeout = setTimeout(async () => {
      loading = true;
      try {
        const data = await api.searchUsers(query.trim());
        // Filtrer les membres déjà sélectionnés
        searchResults = (data.users || []).filter(u => !selectedMembers.find(m => m.id === u.id));
      } catch { searchResults = []; }
      loading = false;
    }, 300);
  }

  function addMember(user) {
    selectedMembers = [...selectedMembers, user];
    searchResults = searchResults.filter(u => u.id !== user.id);
    query = '';
    searchResults = [];
  }

  function removeMember(user) {
    selectedMembers = selectedMembers.filter(m => m.id !== user.id);
  }

  async function handleCreate() {
    if (!groupName.trim() || selectedMembers.length === 0) {
      error = 'Nom du groupe et au moins un membre requis';
      return;
    }
    creating = true;
    error = '';
    try {
      const data = await api.createGroup(groupName.trim(), selectedMembers.map(m => m.id));
      dispatch('created', { group: data });
    } catch (e) {
      error = e.message || 'Erreur lors de la création du groupe';
    }
    creating = false;
  }

  function close() { dispatch('close'); }

  function handleKeydown(e) {
    if (e.key === 'Escape') close();
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 animate-fade-in" on:click|self={close}>
  <div class="bg-slate-800 rounded-2xl shadow-2xl border border-slate-700/50 w-full max-w-md animate-slide-up">
    <!-- En-tête -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-slate-700/50">
      <h3 class="text-lg font-semibold text-white">Créer un groupe</h3>
      <button on:click={close} class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-700 transition-colors">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
      </button>
    </div>

    <div class="px-6 py-4 space-y-4">
      <!-- Nom du groupe -->
      <div>
        <label for="groupName" class="block text-sm font-medium text-slate-300 mb-1.5">Nom du groupe</label>
        <input
          id="groupName"
          type="text"
          bind:value={groupName}
          placeholder="Ex : Projet Alpha"
          class="w-full px-4 py-2.5 bg-slate-900/50 border border-slate-600/50 rounded-xl text-sm text-white
                 placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
        />
      </div>

      <!-- Membres sélectionnés (chips) -->
      {#if selectedMembers.length > 0}
        <div class="flex flex-wrap gap-2">
          {#each selectedMembers as member}
            <span class="inline-flex items-center gap-1.5 pl-3 pr-1.5 py-1 bg-primary-500/20 text-primary-300 rounded-full text-sm">
              {member.username}
              <button on:click={() => removeMember(member)} class="hover:text-red-400 p-0.5 rounded-full hover:bg-red-500/20 transition-colors">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
              </button>
            </span>
          {/each}
        </div>
      {/if}

      <!-- Recherche de membres -->
      <div>
        <label for="memberSearch" class="block text-sm font-medium text-slate-300 mb-1.5">Ajouter des membres</label>
        <div class="relative">
          <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>
          <input
            id="memberSearch"
            type="text"
            bind:value={query}
            on:input={handleSearch}
            placeholder="Rechercher un utilisateur..."
            class="w-full pl-10 pr-4 py-2.5 bg-slate-900/50 border border-slate-600/50 rounded-xl text-sm text-white
                   placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
          />
        </div>
      </div>

      <!-- Résultats de recherche -->
      {#if searchResults.length > 0}
        <div class="max-h-40 overflow-y-auto space-y-1">
          {#each searchResults as user}
            <button
              on:click={() => addMember(user)}
              class="w-full flex items-center gap-3 px-3 py-2 rounded-xl hover:bg-slate-700/50 transition-colors text-left"
            >
              <div class="w-8 h-8 rounded-full bg-primary-500 flex items-center justify-center text-white text-xs font-semibold">
                {(user.username || '?')[0].toUpperCase()}
              </div>
              <span class="text-sm text-white">{user.username}</span>
              <svg class="w-4 h-4 text-emerald-400 ml-auto" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M12 4v16m8-8H4"/></svg>
            </button>
          {/each}
        </div>
      {/if}

      {#if error}
        <p class="text-sm text-red-400 bg-red-500/10 rounded-xl px-4 py-2">{error}</p>
      {/if}
    </div>

    <!-- Footer -->
    <div class="px-6 py-4 border-t border-slate-700/50 flex gap-3">
      <button on:click={close} class="flex-1 py-2.5 text-sm font-medium text-slate-300 bg-slate-700 hover:bg-slate-600 rounded-xl transition-colors">
        Annuler
      </button>
      <button
        on:click={handleCreate}
        disabled={!groupName.trim() || selectedMembers.length === 0 || creating}
        class="flex-1 py-2.5 text-sm font-medium text-white bg-primary-500 hover:bg-primary-600 disabled:bg-slate-700 disabled:text-slate-500
               rounded-xl transition-colors disabled:cursor-not-allowed"
      >
        {#if creating}Création...{:else}Créer le groupe{/if}
      </button>
    </div>
  </div>
</div>
