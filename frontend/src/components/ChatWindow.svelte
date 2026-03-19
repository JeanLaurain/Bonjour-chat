<!--
  ChatWindow — Zone de conversation : en-tête, messages, zone de saisie.
  Gère l'affichage des messages DM et de groupe, les images, les fichiers,
  les séparateurs de date, la pagination, les réponses (swipe) et les appels.
-->
<script>
  import { createEventDispatcher, afterUpdate, tick, onMount } from 'svelte';
  import { auth, currentConversation, onlineUsers } from '../lib/stores.js';
  import MessageInput from './MessageInput.svelte';

  export let messages = [];
  export let conversationType = null;
  export let hasMore = false;
  export let loadingMore = false;
  export let replyTo = null;

  const dispatch = createEventDispatcher();
  let messagesEl;
  let prevLen = 0;
  let lightboxSrc = null;
  let wasAtBottom = true;

  // État du swipe pour répondre
  let swipeState = { active: false, msgId: null, startX: 0, currentX: 0 };

  // Détecter le scroll en haut pour charger les messages plus anciens
  function handleScroll() {
    if (!messagesEl) return;
    if (messagesEl.scrollTop < 50 && hasMore && !loadingMore) {
      dispatch('loadMore');
    }
  }

  afterUpdate(async () => {
    if (messages.length !== prevLen) {
      const added = messages.length - prevLen;
      const wasLoadingOlder = prevLen > 0 && added > 0 && !wasAtBottom;
      prevLen = messages.length;
      await tick();
      if (messagesEl) {
        if (wasLoadingOlder) {
          // Quand on charge des messages plus anciens, maintenir la position de scroll
        } else {
          messagesEl.scrollTo({ top: messagesEl.scrollHeight, behavior: 'smooth' });
        }
      }
    }
  });

  function trackScrollPosition() {
    if (!messagesEl) return;
    wasAtBottom = messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight < 100;
    handleScroll();
  }

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

  function isMine(m) { return m.sender_id === me; }

  function getImageUrl(m) {
    if (m.message_type === 'image' && m.image_url) return m.image_url;
    if (m.message_type === 'image' && m.content && m.content.startsWith('/uploads/')) return m.content;
    return null;
  }

  /** Trouve le message référencé par reply_to_id */
  function getRepliedMessage(replyId) {
    if (!replyId) return null;
    return messages.find(m => m.id === replyId) || null;
  }

  /** Texte résumé pour le message répondu */
  function replyPreviewText(msg) {
    if (!msg) return 'Message supprimé';
    if (msg.message_type === 'image') return '📷 Photo';
    if (msg.message_type === 'file') return `📎 ${msg.original_filename || 'Fichier'}`;
    return msg.content?.substring(0, 60) || '';
  }

  /** Nom de l'expéditeur du message répondu */
  function replySenderName(msg) {
    if (!msg) return '';
    if (msg.sender_id === me) return 'Vous';
    return msg.sender_username || conv?.username || '';
  }

  // ── Swipe-to-reply (gestion tactile) ──
  function handleTouchStart(e, msg) {
    const touch = e.touches[0];
    swipeState = { active: true, msgId: msg.id, startX: touch.clientX, currentX: touch.clientX };
  }

  function handleTouchMove(e) {
    if (!swipeState.active) return;
    const touch = e.touches[0];
    const dx = touch.clientX - swipeState.startX;
    // On ne permet que le swipe vers la droite
    swipeState.currentX = Math.max(0, Math.min(dx, 80));
  }

  function handleTouchEnd() {
    if (!swipeState.active) return;
    const dx = swipeState.currentX;
    if (dx > 50) {
      // Seuil atteint → déclencher la réponse
      const msg = messages.find(m => m.id === swipeState.msgId);
      if (msg) dispatch('reply', msg);
    }
    swipeState = { active: false, msgId: null, startX: 0, currentX: 0 };
  }

  /** Calcule le décalage horizontal pour l'animation de swipe */
  function getSwipeOffset(msgId) {
    if (swipeState.active && swipeState.msgId === msgId) return swipeState.currentX;
    return 0;
  }

  /** Vérifie si le contenu est un chemin de fichier (pour ne pas l'afficher comme texte) */
  function isFilePath(content) {
    return content && content.startsWith('/uploads/');
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
          {#if conv.type === 'dm' && conv.profile_picture_url}
            <img src={conv.profile_picture_url} alt="" class="w-9 h-9 rounded-full object-cover" />
          {:else}
            <div class="w-9 h-9 rounded-full bg-primary-500 flex items-center justify-center text-white text-sm font-semibold">
              {#if conv.type === 'group'}
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2"/><circle cx="9" cy="7" r="4"/></svg>
              {:else}
                {(conv.username || '?')[0].toUpperCase()}
              {/if}
            </div>
          {/if}
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

      <!-- Boutons d'appel (audio/vidéo) -->
      {#if conv.type === 'dm'}
        <button
          on:click={() => dispatch('startCall', { video: false })}
          class="text-slate-400 hover:text-emerald-400 p-2 rounded-lg hover:bg-slate-700/50 transition-colors"
          title="Appel audio"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path d="M3 5a2 2 0 012-2h3.28a1 1 0 01.948.684l1.498 4.493a1 1 0 01-.502 1.21l-2.257 1.13a11.042 11.042 0 005.516 5.516l1.13-2.257a1 1 0 011.21-.502l4.493 1.498a1 1 0 01.684.949V19a2 2 0 01-2 2h-1C9.716 21 3 14.284 3 6V5z"/>
          </svg>
        </button>
        <button
          on:click={() => dispatch('startCall', { video: true })}
          class="text-slate-400 hover:text-emerald-400 p-2 rounded-lg hover:bg-slate-700/50 transition-colors"
          title="Appel vidéo"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"/>
          </svg>
        </button>
      {:else if conv.type === 'group'}
        <!-- En groupe : ouvrir un sélecteur de membre à appeler -->
        <button
          on:click={() => dispatch('groupCall', { video: false })}
          class="text-slate-400 hover:text-emerald-400 p-2 rounded-lg hover:bg-slate-700/50 transition-colors"
          title="Appeler un membre"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path d="M3 5a2 2 0 012-2h3.28a1 1 0 01.948.684l1.498 4.493a1 1 0 01-.502 1.21l-2.257 1.13a11.042 11.042 0 005.516 5.516l1.13-2.257a1 1 0 011.21-.502l4.493 1.498a1 1 0 01.684.949V19a2 2 0 01-2 2h-1C9.716 21 3 14.284 3 6V5z"/>
          </svg>
        </button>
        <button
          on:click={() => dispatch('groupCall', { video: true })}
          class="text-slate-400 hover:text-emerald-400 p-2 rounded-lg hover:bg-slate-700/50 transition-colors"
          title="Appeler un membre (vidéo)"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"/>
          </svg>
        </button>
      {/if}

      <!-- Bouton réglages du groupe -->
      {#if conv.type === 'group'}
        <button
          on:click={() => dispatch('openGroupSettings')}
          class="text-slate-400 hover:text-white p-2 rounded-lg hover:bg-slate-700/50 transition-colors"
          title="Réglages du groupe"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4"/>
          </svg>
        </button>
      {/if}
    </div>

    <!-- Zone des messages avec scroll et détection du haut -->
    <div bind:this={messagesEl} on:scroll={trackScrollPosition} class="flex-1 overflow-y-auto px-4 md:px-6 py-4 space-y-1">
      <!-- Indicateur de chargement des messages plus anciens -->
      {#if loadingMore}
        <div class="flex justify-center py-3">
          <svg class="animate-spin h-5 w-5 text-primary-400" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"/>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
          </svg>
        </div>
      {:else if hasMore}
        <div class="flex justify-center py-2">
          <button on:click={() => dispatch('loadMore')} class="text-xs text-slate-500 hover:text-primary-400 transition-colors">
            ↑ Charger les messages précédents
          </button>
        </div>
      {/if}

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
          {@const replied = getRepliedMessage(item.reply_to_id)}
          {@const offset = getSwipeOffset(item.id)}
          <!-- Message avec support swipe-to-reply -->
          <div
            class="flex {mine ? 'justify-end' : 'justify-start'} animate-fade-in"
            style="transform: translateX({offset}px); transition: {swipeState.active && swipeState.msgId === item.id ? 'none' : 'transform 0.2s'}"
            on:touchstart={(e) => handleTouchStart(e, item)}
            on:touchmove={handleTouchMove}
            on:touchend={handleTouchEnd}
          >
            <!-- Icône de réponse (visible pendant le swipe) -->
            {#if offset > 10}
              <div class="flex items-center mr-2 text-primary-400 opacity-{Math.min(offset / 50, 1)}">
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/></svg>
              </div>
            {/if}

            <div class="max-w-[85%] md:max-w-[65%]">
              <!-- Nom de l'expéditeur (groupes uniquement) -->
              {#if conv.type === 'group' && !mine && item.sender_username}
                <p class="text-[11px] text-primary-400 font-medium mb-0.5 ml-3">{item.sender_username}</p>
              {/if}

              <!-- Aperçu du message auquel celui-ci répond -->
              {#if item.reply_to_id}
                <div class="mx-3 mb-1 px-3 py-1.5 bg-slate-600/30 border-l-2 border-primary-500/50 rounded-r-lg cursor-pointer"
                     on:click={() => {
                       const el = document.getElementById(`msg-${item.reply_to_id}`);
                       if (el) { el.scrollIntoView({ behavior: 'smooth', block: 'center' }); el.classList.add('ring-2', 'ring-primary-500/50'); setTimeout(() => el.classList.remove('ring-2', 'ring-primary-500/50'), 1500); }
                     }}
                >
                  <p class="text-[10px] text-primary-400 font-medium">{replySenderName(replied)}</p>
                  <p class="text-[11px] text-slate-400 truncate">{replyPreviewText(replied)}</p>
                </div>
              {/if}

              <div id="msg-{item.id}" class="rounded-2xl px-4 py-2.5 {mine ? 'bg-primary-500 text-white rounded-br-md' : 'bg-slate-700 text-slate-100 rounded-bl-md'} shadow-sm transition-all">
                {#if img}
                  <!-- Message image -->
                  <button on:click={() => lightboxSrc = img} class="block">
                    <img src={img} alt="Image" class="max-w-full max-h-64 rounded-lg cursor-pointer hover:opacity-90 transition-opacity" loading="lazy" />
                  </button>
                  {#if item.content && !isFilePath(item.content)}
                    <p class="text-sm mt-2 break-words">{item.content}</p>
                  {/if}
                {:else if item.message_type === 'file'}
                  <!-- Message fichier (non-image) -->
                  <a href={item.image_url || item.content} target="_blank" rel="noopener"
                     class="flex items-center gap-3 py-1 {mine ? 'text-white hover:text-primary-100' : 'text-slate-100 hover:text-primary-300'} transition-colors">
                    <svg class="w-8 h-8 flex-shrink-0 opacity-70" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                      <path d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
                    </svg>
                    <span class="text-sm underline truncate">{item.original_filename || 'Télécharger le fichier'}</span>
                  </a>
                  {#if item.content && !isFilePath(item.content)}
                    <p class="text-sm mt-1 break-words">{item.content}</p>
                  {/if}
                {:else}
                  <!-- Message texte -->
                  <p class="text-sm break-words whitespace-pre-wrap">{item.content}</p>
                {/if}
                <p class="text-[10px] {mine ? 'text-primary-200' : 'text-slate-400'} mt-1 text-right">{formatTime(item.created_at)}</p>
              </div>

              <!-- Bouton répondre (clic long / desktop) -->
              <button
                on:click={() => dispatch('reply', item)}
                class="ml-3 mt-0.5 text-[11px] text-slate-500 hover:text-primary-400 transition-colors hidden md:inline-block"
              >↩ Répondre</button>
            </div>
          </div>
        {/if}
      {/each}
    </div>

    <!-- Zone de saisie avec reply context -->
    <MessageInput {replyTo} on:send on:clearReply />

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
