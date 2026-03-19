<!--
  GroupCallPicker — Modal pour sélectionner un membre du groupe à appeler.
  Affiche la liste des membres (hors soi-même) avec possibilité de lancer
  un appel audio ou vidéo vers chacun.
-->
<script>
  import { createEventDispatcher, onMount } from 'svelte';
  import { auth, onlineUsers } from '../lib/stores.js';
  import { getGroup } from '../lib/api.js';

  export let groupId;
  export let video = false;

  const dispatch = createEventDispatcher();
  let members = [];
  let loading = true;

  onMount(async () => {
    try {
      const res = await getGroup(groupId);
      const me = $auth.user?.id;
      // Exclure l'utilisateur actuel de la liste
      members = (res.group?.members || []).filter(m => m.id !== me);
    } catch (e) {
      console.error('Failed to load group members', e);
    }
    loading = false;
  });

  function close() {
    dispatch('close');
  }

  function pickMember(member) {
    dispatch('pick', { userId: member.id, username: member.username, video });
  }
</script>

<!-- Overlay -->
<div class="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 animate-fade-in"
     on:click|self={close} role="dialog" aria-modal="true">
  <div class="bg-slate-800 rounded-2xl shadow-2xl border border-slate-700/50 w-full max-w-sm animate-slide-up">
    <!-- Header -->
    <div class="p-4 border-b border-slate-700/50 flex items-center justify-between">
      <h3 class="text-white font-semibold">
        {video ? '📹 Appel vidéo' : '📞 Appel audio'} — Choisir un membre
      </h3>
      <button on:click={close} class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-700/50 transition-colors">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
      </button>
    </div>

    <!-- Liste des membres -->
    <div class="p-2 max-h-80 overflow-y-auto">
      {#if loading}
        <div class="text-center py-8 text-slate-400">Chargement...</div>
      {:else if members.length === 0}
        <div class="text-center py-8 text-slate-400">Aucun autre membre dans ce groupe</div>
      {:else}
        {#each members as member}
          {@const isOnline = $onlineUsers.has(member.id)}
          <button
            on:click={() => pickMember(member)}
            class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-slate-700/50 transition-colors text-left"
          >
            <!-- Avatar -->
            <div class="relative flex-shrink-0">
              {#if member.profile_picture_url}
                <img src={member.profile_picture_url} alt="" class="w-10 h-10 rounded-full object-cover" />
              {:else}
                <div class="w-10 h-10 rounded-full bg-primary-500 flex items-center justify-center text-white text-sm font-semibold">
                  {(member.username || '?')[0].toUpperCase()}
                </div>
              {/if}
              {#if isOnline}
                <div class="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-emerald-500 border-2 border-slate-800 rounded-full"></div>
              {/if}
            </div>
            <!-- Nom + statut -->
            <div class="flex-1 min-w-0">
              <p class="text-white text-sm font-medium truncate">{member.username}</p>
              <p class="text-xs {isOnline ? 'text-emerald-400' : 'text-slate-500'}">{isOnline ? 'En ligne' : 'Hors ligne'}</p>
            </div>
            <!-- Icône d'appel -->
            <div class="text-emerald-400">
              {#if video}
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"/>
                </svg>
              {:else}
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path d="M3 5a2 2 0 012-2h3.28a1 1 0 01.948.684l1.498 4.493a1 1 0 01-.502 1.21l-2.257 1.13a11.042 11.042 0 005.516 5.516l1.13-2.257a1 1 0 011.21-.502l4.493 1.498a1 1 0 01.684.949V19a2 2 0 01-2 2h-1C9.716 21 3 14.284 3 6V5z"/>
                </svg>
              {/if}
            </div>
          </button>
        {/each}
      {/if}
    </div>
  </div>
</div>
