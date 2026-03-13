<!--
  ChatWindow — Zone de conversation : en-tête, messages, zone de saisie.
  Gère l'affichage des messages DM et de groupe, les images, les séparateurs de date.
-->
<script>
  import { createEventDispatcher, afterUpdate, tick } from 'svelte';
  import { auth, currentConversation, onlineUsers } from '../lib/stores.js';
  import MessageInput from './MessageInput.svelte';

  export let messages = [];
  export let conversationType = null;

  const dispatch = createEventDispatcher();
  let messagesEl;
  let prevLen = 0;
  let lightboxSrc = null;

  afterUpdate(() => {
    if (messages.length !== prevLen) {
      prevLen = messages.length;
      tick().then(() => { messagesEl?.scrollTo({ top: messagesEl.scrollHeight, behavior: 'smooth' }); });
    }
  });

  $: conv = $currentConversation;
  $: online = conv?.type === 'dm' && $onlineUsers[conv?.user_id];
  $: me = $auth.user?.id;

  function formatDate(ts) {
    if (!ts) return '';
    const d = new Date(ts);
    const now = new Date();
    if (d.toDateString() === now.toDateString()) return "Aujourd'hui";
    const y = new Date(now); y.setDate(y.getDate() - 1);
    if (d.toDateString() === y.toDateString()) return 'Hier';
    return d.toLocaleDateString('fr-FR', { day: 'numeric', month: 'long', year: d.getFullYear() !== now.getFullYear() ? 'numeric' : undefined });
  }

  function formatTime(ts) {
    if (!ts) return '';
    return new Date(ts).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
  }

  // Groupement par date pour séparateurs
  function groupByDate(msgs) {
    const groups = [];
    let lastDate = null;
    for (const m of msgs) {
      const d = formatDate(m.created_at);
      if (d !== lastDate) { groups.push({ type: 'date', label: d }); lastDate = d; }
      groups.push({ type: 'msg', ...m });
    }
    return groups;
  }

  $: grouped = groupByDate(messages);

  function isMine(m) {
    return m.sender_id === me;
  }

  function getImageUrl(m) {
    // Priorité à image_url si présent
    if (m.image_url) return m.image_url;
    // Sinon vérifier si le contenu est une URL d'image uploadée
    if (m.message_type === 'image' && m.content) return m.content;
    return null;
  }
</script>

<div class="flex flex-col flex-1 h-full bg-slate-800">
  {#if conv}
    <!-- En-tête de la conversation -->
    <div class="flex items-center gap-3 px-4 md:px-6 py-3 bg-slate-800/90 backdrop-blur border-b border-slate-700/50 flex-shrink-0">
      <!-- Bouton retour (mobile) -->
      <button on:click={() => dispatch('back')} class="md:hidden text-slate-400 hover:text-white p-1.5 -ml-1 rounded-lg hover:bg-slate-700/50 transition-colors">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M15 19l-7-7 7-7"/></svg>
      </button>

      <!-- Avatar + info -->
      <div class="flex items-center gap-3 flex-1 min-w-0">
        <div class="relative">
          <div class="w-9 h-9 rounded-full bg-primary-500 flex items-center justify-center text-white text-sm font-semibold">
            {#if conv.type === 'group'}
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/></svg>
            {:else}
              {(conv.username || '?')[0].toUpperCase()}
            {/if}
          </div>
          {#if online}
            <div class="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-emerald-500 border-2 border-slate-800 rounded-full"></div>
          {/if}
        </div>
        <div class="min-w-0">
          <h3 class="text-white font-semibold text-sm truncate">{conv.username || conv.group_name || ''}</h3>
          {#if conv.type === 'group'}
            <p class="text-xs text-slate-400">{conv.member_count || ''} membres</p>
          {:else if online}
            <p class="text-xs text-emerald-400">En ligne</p>
          {:else if conv.last_seen}
            <p class="text-xs text-slate-400">Vu à {formatTime(conv.last_seen)}</p>
          {/if}
        </div>
      </div>
    </div>

    <!-- Zone des messages avec scroll -->
    <div bind:this={messagesEl} class="flex-1 overflow-y-auto px-4 md:px-6 py-4 space-y-1">
      {#each grouped as item}
        {#if item.type === 'date'}
          <!-- Séparateur de date -->
          <div class="flex items-center gap-4 my-4">
            <div class="flex-1 h-px bg-slate-700/50"></div>
            <span class="text-xs text-slate-500 font-medium">{item.label}</span>
            <div class="flex-1 h-px bg-slate-700/50"></div>
          </div>
        {:else}
          {@const mine = isMine(item)}
          {@const img = getImageUrl(item)}
          <div class="flex {mine ? 'justify-end' : 'justify-start'} animate-fade-in">
            <div class="max-w-[85%] md:max-w-[65%]">
              <!-- Nom de l'expéditeur (groupes uniquement) -->
              {#if conv.type === 'group' && !mine && item.sender_username}
                <p class="text-[11px] text-primary-400 font-medium mb-0.5 ml-3">{item.sender_username}</p>
              {/if}
              <div class="rounded-2xl px-4 py-2.5 {mine ? 'bg-primary-500 text-white rounded-br-md' : 'bg-slate-700 text-slate-100 rounded-bl-md'} shadow-sm">
                {#if img}
                  <!-- Message image -->
                  <button on:click={() => lightboxSrc = img} class="block">
                    <img src={img} alt="Image" class="max-w-full max-h-64 rounded-lg cursor-pointer hover:opacity-90 transition-opacity" loading="lazy" />
                  </button>
                  {#if item.content && item.content !== img && !item.content.startsWith('/uploads/')}
                    <p class="text-sm mt-2 break-words">{item.content}</p>
                  {/if}
                {:else}
                  <!-- Message texte -->
                  <p class="text-sm break-words whitespace-pre-wrap">{item.content}</p>
                {/if}
                <p class="text-[10px] {mine ? 'text-primary-200' : 'text-slate-400'} mt-1 text-right">{formatTime(item.created_at)}</p>
              </div>
            </div>
          </div>
        {/if}
      {/each}
    </div>

    <!-- Zone de saisie -->
    <MessageInput on:send />

  {:else}
    <!-- État vide : aucune conversation sélectionnée -->
    <div class="flex-1 flex flex-col items-center justify-center text-slate-500 p-8">
      <div class="w-20 h-20 rounded-full bg-slate-700/50 flex items-center justify-center mb-6">
        <svg class="w-10 h-10 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/>
        </svg>
      </div>
      <h3 class="text-xl font-semibold text-slate-300 mb-2">Bienvenue sur Bonjour</h3>
      <p class="text-sm text-center max-w-sm">Sélectionnez une conversation ou démarrez-en une nouvelle depuis la barre latérale.</p>
    </div>
  {/if}
</div>

<!-- Lightbox plein écran pour les images -->
{#if lightboxSrc}
  <button class="fixed inset-0 z-50 bg-black/90 flex items-center justify-center p-4 cursor-zoom-out" on:click={() => lightboxSrc = null}>
    <img src={lightboxSrc} alt="Image agrandie" class="max-w-full max-h-full object-contain rounded-lg shadow-2xl" />
    <button on:click|stopPropagation={() => lightboxSrc = null} class="absolute top-4 right-4 text-white/70 hover:text-white bg-black/50 rounded-full p-2">
      <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
    </button>
  </button>
{/if}
