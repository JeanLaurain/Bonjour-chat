<!--
  Sidebar — Barre latérale collapsible avec la liste unifiée DM + groupes.
  Affiche avatars, noms, derniers messages, badges non lus et statuts en ligne.
-->
<script>
  import { createEventDispatcher } from 'svelte';
  import { auth, sidebarCollapsed, unreadCounts, groupUnreadCounts, onlineUsers, currentConversation } from '../lib/stores.js';

  export let conversations = [];

  const dispatch = createEventDispatcher();
  let collapsed = false;
  sidebarCollapsed.subscribe(v => collapsed = v);

  // ID de la conversation sélectionnée pour le surlignage actif
  $: activeKey = $currentConversation
    ? ($currentConversation.type === 'group'
      ? `group-${$currentConversation.group_id || $currentConversation.id}`
      : `dm-${$currentConversation.user_id}`)
    : null;

  function getKey(conv) {
    return conv.type === 'group' ? `group-${conv.group_id || conv.id}` : `dm-${conv.user_id}`;
  }

  function getUnread(conv) {
    if (conv.type === 'group') return $groupUnreadCounts[conv.group_id || conv.id] || 0;
    return $unreadCounts[conv.user_id] || 0;
  }

  function isOnline(conv) {
    if (conv.type === 'group') return false;
    return $onlineUsers[conv.user_id] || false;
  }

  function formatTime(ts) {
    if (!ts) return '';
    const d = new Date(ts);
    const now = new Date();
    if (d.toDateString() === now.toDateString()) return d.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
    const diff = now.getTime() - d.getTime();
    if (diff < 7 * 86400000) return d.toLocaleDateString('fr-FR', { weekday: 'short' });
    return d.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit' });
  }

  function getInitials(name) {
    if (!name) return '?';
    return name.split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase();
  }

  // Couleur avatar déterministe basée sur le nom
  const colors = ['bg-primary-500', 'bg-emerald-500', 'bg-amber-500', 'bg-rose-500', 'bg-cyan-500', 'bg-violet-500', 'bg-orange-500'];
  function avatarColor(name) {
    let hash = 0;
    for (let i = 0; i < (name?.length || 0); i++) hash = name.charCodeAt(i) + ((hash << 5) - hash);
    return colors[Math.abs(hash) % colors.length];
  }

  function handleLogout() { auth.logout(); }

  $: totalUnread = Object.values($unreadCounts).reduce((s, v) => s + v, 0)
    + Object.values($groupUnreadCounts).reduce((s, v) => s + v, 0);
</script>

<aside
  class="flex flex-col bg-slate-900 border-r border-slate-700/50 h-full transition-all duration-300 ease-in-out overflow-hidden
         {collapsed ? 'w-[72px]' : 'w-80'}"
>
  <!-- En-tête : Logo, titre, bouton collapse -->
  <div class="flex items-center {collapsed ? 'flex-col gap-2 px-2 py-3' : 'gap-3 px-4 py-4'} border-b border-slate-700/50 flex-shrink-0">
    <div class="flex-shrink-0 w-10 h-10 rounded-xl bg-primary-500 flex items-center justify-center text-white text-lg">
      💬
    </div>
    {#if !collapsed}
      <div class="flex-1 min-w-0">
        <h2 class="text-white font-bold text-lg leading-tight">Bonjour</h2>
        <p class="text-slate-400 text-xs truncate">{$auth.user?.username || ''}</p>
      </div>
    {/if}
    <button on:click={() => sidebarCollapsed.toggle()} class="text-slate-400 hover:text-white p-1.5 rounded-lg hover:bg-slate-800 transition-colors flex-shrink-0" title="{collapsed ? 'Ouvrir' : 'Réduire'}">
      <svg class="w-5 h-5 transition-transform {collapsed ? 'rotate-180' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path d="M11 19l-7-7 7-7M18 19l-7-7 7-7"/>
      </svg>
    </button>
  </div>

  <!-- Boutons d'action : nouvelle conversation, nouveau groupe -->
  {#if !collapsed}
    <div class="flex gap-2 px-4 py-3 border-b border-slate-700/50 flex-shrink-0">
      <button on:click={() => dispatch('newConversation')} class="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-primary-500/10 hover:bg-primary-500/20 text-primary-400 rounded-xl text-sm font-medium transition-colors">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M12 4v16m8-8H4"/></svg>
        Message
      </button>
      <button on:click={() => dispatch('newGroup')} class="flex-1 flex items-center justify-center gap-2 px-3 py-2 bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-400 rounded-xl text-sm font-medium transition-colors">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75"/></svg>
        Groupe
      </button>
    </div>
  {:else}
    <div class="flex flex-col items-center gap-2 py-3 border-b border-slate-700/50 flex-shrink-0">
      <button on:click={() => dispatch('newConversation')} class="p-2 rounded-xl bg-primary-500/10 text-primary-400 hover:bg-primary-500/20 transition-colors" title="Nouveau message">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M12 4v16m8-8H4"/></svg>
      </button>
      <button on:click={() => dispatch('newGroup')} class="p-2 rounded-xl bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 transition-colors" title="Nouveau groupe">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/></svg>
      </button>
    </div>
  {/if}

  <!-- Liste des conversations -->
  <div class="flex-1 overflow-y-auto">
    {#if conversations.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-slate-500 text-sm px-4 text-center">
        {#if !collapsed}
          <svg class="w-12 h-12 mb-3 text-slate-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/></svg>
          <p>Aucune conversation</p>
          <p class="text-xs text-slate-600 mt-1">Cliquez sur "Message" pour commencer</p>
        {/if}
      </div>
    {:else}
      {#each conversations as conv (getKey(conv))}
        {@const unread = getUnread(conv)}
        {@const online = isOnline(conv)}
        {@const active = getKey(conv) === activeKey}
        <button
          on:click={() => dispatch('select', conv)}
          class="w-full flex items-center gap-3 px-4 py-3 transition-colors text-left
                 {active ? 'bg-primary-500/15 border-l-2 border-primary-500' : 'border-l-2 border-transparent hover:bg-slate-800/70'}
                 {collapsed ? 'justify-center px-0' : ''}"
        >
          <!-- Avatar -->
          <div class="relative flex-shrink-0">
            <div class="w-10 h-10 rounded-full {avatarColor(conv.username || conv.group_name)} flex items-center justify-center text-white text-sm font-semibold">
              {#if conv.type === 'group'}
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75"/></svg>
              {:else}
                {getInitials(conv.username)}
              {/if}
            </div>
            <!-- Pastille en ligne -->
            {#if online}
              <div class="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 bg-emerald-500 border-2 border-slate-900 rounded-full"></div>
            {/if}
            <!-- Badge non lu (mode collapsed) -->
            {#if collapsed && unread > 0}
              <div class="absolute -top-1 -right-1 min-w-[18px] h-[18px] bg-primary-500 rounded-full text-[10px] font-bold text-white flex items-center justify-center px-1">
                {unread > 99 ? '99+' : unread}
              </div>
            {/if}
          </div>

          <!-- Info conversation (mode expanded) -->
          {#if !collapsed}
            <div class="flex-1 min-w-0">
              <div class="flex items-center justify-between">
                <span class="text-sm font-medium {active ? 'text-white' : 'text-slate-200'} truncate">
                  {conv.username || conv.group_name}
                </span>
                <span class="text-[11px] text-slate-500 flex-shrink-0 ml-2">
                  {formatTime(conv.last_message_at)}
                </span>
              </div>
              <div class="flex items-center justify-between mt-0.5">
                <p class="text-xs text-slate-400 truncate flex-1">
                  {conv.last_message || (conv.type === 'group' ? `${conv.member_count || 0} membres` : 'Commencer la conversation')}
                </p>
                {#if unread > 0}
                  <span class="ml-2 min-w-[20px] h-5 bg-primary-500 rounded-full text-[11px] font-bold text-white flex items-center justify-center px-1.5 flex-shrink-0">
                    {unread > 99 ? '99+' : unread}
                  </span>
                {/if}
              </div>
              <!-- Dernier vu (DM uniquement) -->
              {#if conv.type === 'dm' && conv.last_seen && !online}
                <p class="text-[10px] text-slate-500 mt-0.5">
                  Vu {formatTime(conv.last_seen)}
                </p>
              {/if}
            </div>
          {/if}
        </button>
      {/each}
    {/if}
  </div>

  <!-- Footer : bouton déconnexion -->
  <div class="border-t border-slate-700/50 p-3 flex-shrink-0">
    <button
      on:click={handleLogout}
      class="w-full flex items-center {collapsed ? 'justify-center' : 'gap-2'} px-3 py-2 text-slate-400 hover:text-red-400 hover:bg-red-500/10 rounded-xl transition-colors text-sm"
    >
      <svg class="w-5 h-5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"/>
      </svg>
      {#if !collapsed}<span>Déconnexion</span>{/if}
    </button>
  </div>
</aside>
