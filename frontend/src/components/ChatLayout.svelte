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

  let conversations = [];
  let groupConversations = [];
  let messages = [];
  let showNewConv = false;
  let showNewGroup = false;
  let showGroupSettings = false;
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

  // État de l'appel WebRTC
  let callState = null; // null | { type: 'outgoing'|'incoming', userId, username, video, offer? }
  let activeCall = false;

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
    if (wsConn && wsConn.readyState === WebSocket.OPEN) {
      wsConn.send(JSON.stringify(msg));
    }
  }

  function handleWsMsg(data) {
    const me = $auth.user?.id;

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
      // Appel entrant
      if (activeCall) {
        // Déjà en appel → refuser automatiquement
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
      // La réponse est gérée par le CallModal via un callback
      callState = { ...callState, answer: data.answer };
    }

    if (data.type === 'ice_candidate' && (callState || activeCall)) {
      // Les candidats ICE sont gérés par le CallModal
      callState = callState ? { ...callState, newIceCandidate: data.candidate } : callState;
    }

    if (data.type === 'call_hangup' || data.type === 'call_reject' || data.type === 'call_busy') {
      callState = null;
      activeCall = false;
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

  /** Démarrer un appel (audio ou vidéo) */
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

  <!-- Modal d'appel WebRTC -->
  {#if callState}
    <CallModal
      {callState}
      {sendWs}
      on:connected={handleCallConnected}
      on:end={handleCallEnd}
    />
  {/if}
</div>
