<!--
  GroupSettings — Modal de gestion d'un groupe.
  Permet de renommer le groupe, voir les membres, en ajouter ou en supprimer.
-->
<script>
  import { createEventDispatcher } from 'svelte';
  import { auth } from '../lib/stores.js';
  import * as api from '../lib/api.js';

  export let groupId;
  export let groupName = '';

  const dispatch = createEventDispatcher();
  let members = [];
  let newName = groupName;
  let searchQuery = '';
  let searchResults = [];
  let loading = false;
  let error = '';
  let success = '';
  let renaming = false;
  let searching = false;

  $: me = $auth.user?.id;

  // Charger les détails du groupe (membres) à l'ouverture
  loadGroup();

  async function loadGroup() {
    try {
      const data = await api.getGroup(groupId);
      members = data.members || [];
      if (data.group?.name) newName = data.group.name;
    } catch (e) {
      error = 'Erreur lors du chargement du groupe';
    }
  }

  async function handleRename() {
    if (!newName.trim() || newName === groupName) return;
    renaming = true;
    error = '';
    try {
      await api.renameGroup(groupId, newName.trim());
      success = 'Groupe renommé !';
      dispatch('renamed', { name: newName.trim() });
      setTimeout(() => success = '', 2000);
    } catch (e) {
      error = e.message || 'Erreur lors du renommage';
    } finally {
      renaming = false;
    }
  }

  async function handleSearch() {
    if (!searchQuery.trim()) { searchResults = []; return; }
    searching = true;
    try {
      const data = await api.searchUsers(searchQuery);
      // Filtrer les utilisateurs déjà membres
      const memberIds = new Set(members.map(m => m.id));
      searchResults = (data.users || []).filter(u => !memberIds.has(u.id));
    } catch (e) {
      searchResults = [];
    } finally {
      searching = false;
    }
  }

  async function addMember(userId) {
    try {
      await api.addGroupMembers(groupId, [userId]);
      await loadGroup();
      searchResults = searchResults.filter(u => u.id !== userId);
      dispatch('membersChanged');
    } catch (e) {
      error = e.message || "Erreur lors de l'ajout";
    }
  }

  async function removeMember(userId) {
    try {
      await api.removeGroupMember(groupId, userId);
      await loadGroup();
      dispatch('membersChanged');
    } catch (e) {
      error = e.message || 'Erreur lors de la suppression';
    }
  }

  function close() {
    dispatch('close');
  }
</script>

<!-- Overlay modal -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" on:click|self={close}>
  <div class="bg-slate-800 rounded-2xl shadow-2xl border border-slate-700/50 w-full max-w-md max-h-[85vh] overflow-hidden flex flex-col animate-fade-in">
    <!-- Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-slate-700/50">
      <h2 class="text-lg font-bold text-white">Réglages du groupe</h2>
      <button on:click={close} class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-700/50 transition-colors">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
      </button>
    </div>

    <div class="flex-1 overflow-y-auto p-6 space-y-6">
      <!-- Section : Renommer le groupe -->
      <div>
        <label class="block text-sm font-medium text-slate-300 mb-2">Nom du groupe</label>
        <div class="flex gap-2">
          <input
            type="text"
            bind:value={newName}
            maxlength="100"
            class="flex-1 px-4 py-2.5 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500
                   focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all text-sm"
            placeholder="Nom du groupe"
          />
          <button
            on:click={handleRename}
            disabled={renaming || !newName.trim() || newName === groupName}
            class="px-4 py-2.5 bg-primary-500 hover:bg-primary-600 disabled:bg-slate-600 text-white text-sm font-medium rounded-xl transition-colors disabled:cursor-not-allowed"
          >
            {renaming ? '...' : 'Renommer'}
          </button>
        </div>
      </div>

      {#if error}
        <div class="bg-red-500/10 border border-red-500/30 text-red-400 text-sm rounded-xl px-4 py-2">{error}</div>
      {/if}
      {#if success}
        <div class="bg-green-500/10 border border-green-500/30 text-green-400 text-sm rounded-xl px-4 py-2">{success}</div>
      {/if}

      <!-- Section : Membres actuels -->
      <div>
        <h3 class="text-sm font-medium text-slate-300 mb-3">Membres ({members.length})</h3>
        <div class="space-y-2 max-h-48 overflow-y-auto">
          {#each members as member}
            <div class="flex items-center justify-between px-3 py-2 rounded-xl bg-slate-700/30 hover:bg-slate-700/50 transition-colors">
              <div class="flex items-center gap-3">
                <div class="w-8 h-8 rounded-full bg-primary-500/30 flex items-center justify-center text-primary-400 text-sm font-semibold">
                  {(member.username || '?')[0].toUpperCase()}
                </div>
                <span class="text-sm text-white">{member.username}</span>
                {#if member.id === me}
                  <span class="text-[10px] bg-primary-500/20 text-primary-400 px-2 py-0.5 rounded-full">Vous</span>
                {/if}
              </div>
              {#if member.id !== me}
                <button
                  on:click={() => removeMember(member.id)}
                  class="text-red-400 hover:text-red-300 p-1.5 rounded-lg hover:bg-red-500/10 transition-colors"
                  title="Retirer du groupe"
                >
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
                </button>
              {/if}
            </div>
          {/each}
        </div>
      </div>

      <!-- Section : Ajouter un membre -->
      <div>
        <h3 class="text-sm font-medium text-slate-300 mb-2">Ajouter un membre</h3>
        <input
          type="text"
          bind:value={searchQuery}
          on:input={handleSearch}
          placeholder="Rechercher un utilisateur..."
          class="w-full px-4 py-2.5 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500
                 focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all text-sm"
        />
        {#if searchResults.length > 0}
          <div class="mt-2 space-y-1 max-h-32 overflow-y-auto">
            {#each searchResults as user}
              <button
                on:click={() => addMember(user.id)}
                class="w-full flex items-center gap-3 px-3 py-2 rounded-xl hover:bg-slate-700/50 transition-colors text-left"
              >
                <div class="w-8 h-8 rounded-full bg-emerald-500/30 flex items-center justify-center text-emerald-400 text-sm font-semibold">
                  {(user.username || '?')[0].toUpperCase()}
                </div>
                <span class="text-sm text-white">{user.username}</span>
                <span class="ml-auto text-xs text-emerald-400">+ Ajouter</span>
              </button>
            {/each}
          </div>
        {:else if searchQuery.trim() && !searching}
          <p class="text-xs text-slate-500 mt-2">Aucun utilisateur trouvé</p>
        {/if}
      </div>
    </div>
  </div>
</div>
