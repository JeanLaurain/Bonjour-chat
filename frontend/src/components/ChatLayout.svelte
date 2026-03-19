<!--
  ChatLayout — Orchestrateur principal après connexion.
  Gère les conversations DM + groupes, le WebSocket, les notifications,
  la pagination des messages, les réglages de groupe, les réponses et les appels.
  Responsive : bascule sidebar/chat sur mobile.
-->
<script>
  import { onMount, onDestroy } from 'svelte';
  import { auth, currentConversation, onlineUsers, unreadCounts, groupUnreadCounts, createWsConnection } from '../lib/stores.js';
  import * as api from '../lib/api.js';
  import { showLocalNotification, requestNotificationPermission, setupPushSubscription } from '../lib/notifications.js';
  import Sidebar from './Sidebar.svelte';
  import ChatWindow from './ChatWindow.svelte';
  import NewConversation from './NewConversation.svelte';
  import CreateGroup from './CreateGroup.svelte';
  import GroupSettings from './GroupSettings.svelte';
  import CallModal from './CallModal.svelte';
  import ProfileModal from './ProfileModal.svelte';
  import GroupCallPicker from './GroupCallPicker.svelte';
  import GroupCallModal from './GroupCallModal.svelte';

  let conversations = [];
  let groupConversations = [];
  let messages = [];
  let showNewConv = false;
  let showNewGroup = false;
  let showGroupSettings = false;
  let showProfile = false;
  let groupCallPicker = null; // null | { video: boolean }
  let wsConn = null;
  let selectedUserId = null;
  let selectedGroupId = null;
  let convType = null; // 'dm' | 'group'
  let mobileShowChat = false;
  let isMobile = false;
  let pollInterval;

  // État de pagination
  let hasMore = false;
  let loadingMore = false;

  // État de la réponse à un message
  let replyTo = null;

  // État de l'appel WebRTC (1-à-1)
  let callState = null; // null | { type: 'outgoing'|'incoming', userId, username, video, offer? }
  let activeCall = false;

  // État de l'appel de groupe (mesh)
  let groupCallState = null; // null | { groupId, video }
  let groupCallSignal = null; // signal entrant pour GroupCallModal
  let incomingGroupCall = null; // notification d'appel entrant { groupId, fromId, username, video }

  function checkMobile() { isMobile = window.innerWidth < 768; }

  onMount(async () => {
    checkMobile();
    window.addEventListener('resize', checkMobile);
    await Promise.all([loadConversations(), loadGroups()]);
    connectWs();
    pollInterval = setInterval(() => { loadConversations(); loadGroups(); }, 30000);
    initUnreadCounts();
    const perm = await requestNotificationPermission();
    if (perm === 'granted') setupPushSubscription().catch(() => {});
  });

  onDestroy(() => {
    wsConn?.close();
    clearInterval(pollInterval);
    window.removeEventListener('resize', checkMobile);
  });

  // Liste unifiée DM + groupes, triée par dernier message
  $: allConversations = mergeConversations(conversations, groupConversations);

  function mergeConversations(dms, groups) {
    const merged = [
      ...dms.map(c => ({ ...c, type: 'dm' })),
      ...groups.map(g => ({
        type: 'group', id: g.id, user_id: null, username: g.name,
        last_message: g.last_sender ? `${g.last_sender}: ${g.last_message || ''}` : (g.last_message || ''),
        last_message_at: g.last_message_at, last_seen: null,
        member_count: g.member_count, unread_count: g.unread_count || 0,
        group_id: g.id, group_name: g.name,
      })),
    ];
    return merged.sort((a, b) => {
      const ta = a.last_message_at ? new Date(a.last_message_at).getTime() : 0;
      const tb = b.last_message_at ? new Date(b.last_message_at).getTime() : 0;
      return tb - ta;
    });
  }

  function initUnreadCounts() {
    const dm = {};
    for (const c of conversations) { if (c.unread_count > 0) dm[c.user_id] = c.unread_count; }
    unreadCounts.set(dm);
    const gr = {};
    for (const g of groupConversations) { if (g.unread_count > 0) gr[g.id] = g.unread_count; }
    groupUnreadCounts.set(gr);
  }

  function connectWs() {
    const s = $auth;
    if (!s.token) return;
    wsConn = createWsConnection(s.token, handleWsMsg);
  }

  /** Envoie un message JSON via WebSocket (pour le signaling WebRTC) */
  function sendWs(msg) {
    wsConn?.send(msg);
  }

  function handleWsMsg(data) {
    const me = $auth.user?.id;

    // Log tous les messages WS pour debug
    if (data.type?.startsWith('call') || data.type === 'ice_candidate') {
      console.log('[WS] ←', data.type, data);
    }

    // Messages de chat
    if (data.type === 'new_message') {
      const m = data.data;
      if (convType === 'dm' && selectedUserId && (m.sender_id === selectedUserId || m.receiver_id === selectedUserId)) {
        if (!messages.find(x => x.id === m.id)) messages = [...messages, m];
        if (m.sender_id === selectedUserId) api.markAsRead(selectedUserId).catch(() => {});
      } else if (m.sender_id !== me) {
        unreadCounts.update(c => ({ ...c, [m.sender_id]: (c[m.sender_id] || 0) + 1 }));
        const name = conversations.find(c => c.user_id === m.sender_id)?.username || 'Nouveau message';
        showLocalNotification(name, m.message_type === 'image' ? '📷 Image' : m.message_type === 'file' ? '📎 Fichier' : m.content);
      }
      loadConversations();
    }

    if (data.type === 'new_group_message') {
      const m = data.data;
      if (convType === 'group' && selectedGroupId === m.group_id) {
        if (!messages.find(x => x.id === m.id)) messages = [...messages, m];
      } else if (m.sender_id !== me) {
        groupUnreadCounts.update(c => ({ ...c, [m.group_id]: (c[m.group_id] || 0) + 1 }));
        const gname = groupConversations.find(g => g.id === m.group_id)?.name || 'Groupe';
        showLocalNotification(gname, `${m.sender_username}: ${m.content}`);
      }
      loadGroups();
    }

    if (data.type === 'group_created') loadGroups();

    if (data.type === 'group_renamed') {
      const { group_id, name } = data.data;
      loadGroups();
      if (convType === 'group' && selectedGroupId === group_id) {
        currentConversation.update(c => c ? { ...c, username: name, group_name: name } : c);
      }
    }

    if (data.type === 'user_status') {
      onlineUsers.update(u => ({ ...u, [data.data.user_id]: data.data.online }));
    }

    // ── WebRTC Signaling ──
    if (data.type === 'call_offer') {
      console.log('[WS] call_offer reçu de user', data.from_id);
      if (activeCall) {
        sendWs({ type: 'call_busy', target_id: data.from_id });
        return;
      }
      const caller = conversations.find(c => c.user_id === data.from_id);
      callState = {
        type: 'incoming',
        userId: data.from_id,
        username: caller?.username || `User #${data.from_id}`,
        video: data.video || false,
        offer: data.offer,
      };
    }

    if (data.type === 'call_answer' && callState?.type === 'outgoing') {
      console.log('[WS] call_answer reçu, answer:', !!data.answer);
      callState = { ...callState, answer: data.answer };
    }

    if (data.type === 'ice_candidate' && (callState || activeCall)) {
      console.log('[WS] ice_candidate reçu');
      // Accumuler les candidats ICE dans un array (jamais en écraser)
      if (callState) {
        const candidates = [...(callState.iceCandidates || []), data.candidate];
        callState = { ...callState, iceCandidates: candidates };
      }
    }

    if (data.type === 'call_hangup' || data.type === 'call_reject' || data.type === 'call_busy') {
      console.log('[WS] call end signal:', data.type);
      callState = null;
      activeCall = false;
    }

    // ── Signalisation appel de groupe ──
    if (['group_call_offer', 'group_call_answer', 'group_ice_candidate', 'group_call_leave', 'group_call_hangup'].includes(data.type)) {
      // Relayer au GroupCallModal si actif
      if (groupCallState) {
        groupCallSignal = { ...data };
      }
    }

    if (data.type === 'group_call_start' || data.type === 'group_call_join') {
      if (groupCallState) {
        // Déjà dans un appel de groupe : relayer comme un join
        groupCallSignal = { ...data };
      } else {
        // Notification d'appel de groupe entrant
        incomingGroupCall = {
          groupId: data.group_id,
          fromId: data.from_id,
          username: data.username || `User #${data.from_id}`,
          video: data.video || false,
        };
      }
    }
  }

  async function loadConversations() {
    try {
      const d = await api.listConversations();
      conversations = d.conversations || [];
    } catch (e) {
      if (e.message?.includes('401') || e.message?.includes('Unauthorized')) auth.logout();
    }
  }

  async function loadGroups() {
    try {
      const d = await api.listGroups();
      groupConversations = d.groups || [];
    } catch {}
  }

  async function loadMessages(userId) {
    try {
      const data = await api.getConversation(userId);
      messages = data.messages || [];
      hasMore = data.has_more || false;
    } catch {
      messages = [];
      hasMore = false;
    }
  }

  async function loadGroupMessages(groupId) {
    try {
      const data = await api.getGroupMessages(groupId);
      messages = data.messages || [];
      hasMore = data.has_more || false;
    } catch {
      messages = [];
      hasMore = false;
    }
  }

  async function handleLoadMore() {
    if (loadingMore || !hasMore || messages.length === 0) return;
    loadingMore = true;
    const oldestId = messages[0]?.id;
    if (!oldestId) { loadingMore = false; return; }

    try {
      let data;
      if (convType === 'dm' && selectedUserId) {
        data = await api.getConversation(selectedUserId, oldestId);
      } else if (convType === 'group' && selectedGroupId) {
        data = await api.getGroupMessages(selectedGroupId, oldestId);
      }
      if (data) {
        const older = data.messages || [];
        messages = [...older, ...messages];
        hasMore = data.has_more || false;
      }
    } catch {} finally {
      loadingMore = false;
    }
  }

  async function selectConversation(conv) {
    hasMore = false;
    loadingMore = false;
    replyTo = null;

    if (conv.type === 'group') {
      selectedGroupId = conv.group_id || conv.id;
      selectedUserId = null;
      convType = 'group';
      currentConversation.set({ ...conv, type: 'group' });
      await loadGroupMessages(selectedGroupId);
      groupUnreadCounts.update(c => { const n = { ...c }; delete n[selectedGroupId]; return n; });
    } else {
      selectedUserId = conv.user_id;
      selectedGroupId = null;
      convType = 'dm';
      currentConversation.set({ ...conv, type: 'dm' });
      await loadMessages(conv.user_id);
      api.markAsRead(conv.user_id).catch(() => {});
      unreadCounts.update(c => { const n = { ...c }; delete n[conv.user_id]; return n; });
    }
    if (isMobile) mobileShowChat = true;
  }

  async function handleSend(e) {
    const { content, messageType, imageUrl, originalFilename, replyToId } = e.detail;
    try {
      if (convType === 'group' && selectedGroupId) {
        await api.sendGroupMessage(selectedGroupId, content, messageType, imageUrl, originalFilename, replyToId);
        await loadGroupMessages(selectedGroupId);
        loadGroups();
      } else if (convType === 'dm' && selectedUserId) {
        await api.sendMessage(selectedUserId, content, messageType, imageUrl, originalFilename, replyToId);
        await loadMessages(selectedUserId);
        loadConversations();
      }
    } catch (err) { console.error('Send error:', err); }
    replyTo = null;
  }

  /** L'utilisateur veut répondre à un message */
  function handleReply(e) {
    replyTo = e.detail;
  }

  function handleClearReply() {
    replyTo = null;
  }

  /** Démarrer un appel (audio ou vidéo) en DM */
  function handleStartCall(e) {
    if (activeCall || !selectedUserId) return;
    const { video } = e.detail;
    const conv = $currentConversation;
    callState = {
      type: 'outgoing',
      userId: selectedUserId,
      username: conv?.username || '',
      video: video || false,
    };
  }

  /** Démarrer un appel de groupe */
  function handleGroupCall(e) {
    if (activeCall || groupCallState) return;
    groupCallState = { groupId: selectedGroupId, video: e.detail.video || false };
  }

  /** Rejoindre un appel de groupe entrant */
  function joinIncomingGroupCall() {
    if (!incomingGroupCall) return;
    groupCallState = { groupId: incomingGroupCall.groupId, video: incomingGroupCall.video };
    incomingGroupCall = null;
  }

  /** Refuser un appel de groupe entrant */
  function dismissGroupCall() {
    incomingGroupCall = null;
  }

  /** Fin d'appel de groupe */
  function handleGroupCallEnd() {
    groupCallState = null;
    groupCallSignal = null;
  }

  /** Fin d'appel depuis le CallModal */
  function handleCallEnd() {
    callState = null;
    activeCall = false;
  }

  function handleCallConnected() {
    activeCall = true;
  }

  async function handleStartConv(e) {
    const { user } = e.detail;
    showNewConv = false;
    selectedUserId = user.id;
    selectedGroupId = null;
    convType = 'dm';
    hasMore = false;
    replyTo = null;
    currentConversation.set({ type: 'dm', user_id: user.id, username: user.username, last_message: '', last_message_at: null, last_seen: user.last_seen });
    await loadMessages(user.id);
    loadConversations();
    if (isMobile) mobileShowChat = true;
  }

  async function handleGroupCreated(e) {
    showNewGroup = false;
    const { group } = e.detail;
    selectedGroupId = group.id;
    selectedUserId = null;
    convType = 'group';
    hasMore = false;
    replyTo = null;
    currentConversation.set({ type: 'group', id: group.id, group_id: group.id, username: group.name, group_name: group.name, last_message: '', member_count: group.members?.length || 0 });
    messages = [];
    loadGroups();
    if (isMobile) mobileShowChat = true;
  }

  function handleGroupRenamed(e) {
    const { name } = e.detail;
    currentConversation.update(c => c ? { ...c, username: name, group_name: name } : c);
    loadGroups();
  }

  function handleBack() { mobileShowChat = false; }
</script>

<div class="flex h-screen bg-slate-900 overflow-hidden">
  <!-- Sidebar — caché sur mobile quand le chat est affiché -->
  <div class="{mobileShowChat && isMobile ? 'hidden' : 'flex'} md:flex flex-shrink-0">
    <Sidebar
      conversations={allConversations}
      on:select={(e) => selectConversation(e.detail)}
      on:newConversation={() => showNewConv = true}
      on:newGroup={() => showNewGroup = true}
      on:openProfile={() => showProfile = true}
    />
  </div>

  <!-- Chat — caché sur mobile quand la sidebar est affichée -->
  <div class="{!mobileShowChat && isMobile ? 'hidden' : 'flex'} md:flex flex-1 min-w-0">
    <ChatWindow
      {messages}
      conversationType={convType}
      {hasMore}
      {loadingMore}
      {replyTo}
      on:send={handleSend}
      on:back={handleBack}
      on:loadMore={handleLoadMore}
      on:openGroupSettings={() => showGroupSettings = true}
      on:reply={handleReply}
      on:clearReply={handleClearReply}
      on:startCall={handleStartCall}
      on:groupCall={handleGroupCall}
    />
  </div>

  {#if showNewConv}
    <NewConversation on:select={handleStartConv} on:close={() => showNewConv = false} />
  {/if}
  {#if showNewGroup}
    <CreateGroup on:created={handleGroupCreated} on:close={() => showNewGroup = false} />
  {/if}
  {#if showGroupSettings && selectedGroupId}
    <GroupSettings
      groupId={selectedGroupId}
      groupName={$currentConversation?.group_name || ''}
      on:renamed={handleGroupRenamed}
      on:membersChanged={() => loadGroups()}
      on:close={() => showGroupSettings = false}
    />
  {/if}

  <!-- Modal d'appel WebRTC (1-à-1) -->
  {#if callState}
    <CallModal
      {callState}
      {sendWs}
      on:connected={handleCallConnected}
      on:end={handleCallEnd}
    />
  {/if}

  <!-- Modal d'appel de groupe (mesh) -->
  {#if groupCallState}
    <GroupCallModal
      groupId={groupCallState.groupId}
      video={groupCallState.video}
      {sendWs}
      incomingSignal={groupCallSignal}
      on:end={handleGroupCallEnd}
    />
  {/if}

  <!-- Notification d'appel de groupe entrant -->
  {#if incomingGroupCall && !groupCallState}
    <div class="fixed top-4 left-1/2 -translate-x-1/2 z-50 bg-slate-800 border border-emerald-500/50 rounded-2xl shadow-2xl p-4 flex items-center gap-4 animate-slide-down max-w-sm">
      <div class="w-12 h-12 rounded-full bg-emerald-500/20 flex items-center justify-center animate-pulse">
        <svg class="w-6 h-6 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path d="M3 5a2 2 0 012-2h3.28a1 1 0 01.948.684l1.498 4.493a1 1 0 01-.502 1.21l-2.257 1.13a11.042 11.042 0 005.516 5.516l1.13-2.257a1 1 0 011.21-.502l4.493 1.498a1 1 0 01.684.949V19a2 2 0 01-2 2h-1C9.716 21 3 14.284 3 6V5z"/>
        </svg>
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-white text-sm font-semibold">Appel de groupe</p>
        <p class="text-slate-400 text-xs truncate">{incomingGroupCall.username} a lancé un appel</p>
      </div>
      <div class="flex gap-2">
        <button on:click={joinIncomingGroupCall} class="px-3 py-1.5 bg-emerald-500 hover:bg-emerald-600 text-white text-sm rounded-lg font-medium transition-colors">
          Rejoindre
        </button>
        <button on:click={dismissGroupCall} class="px-3 py-1.5 bg-slate-700 hover:bg-slate-600 text-white text-sm rounded-lg transition-colors">
          Ignorer
        </button>
      </div>
    </div>
  {/if}

  <!-- Modal de profil -->
  {#if showProfile}
    <ProfileModal on:close={() => showProfile = false} />
  {/if}
</div>
