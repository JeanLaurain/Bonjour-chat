<!--
  GroupCallModal — Appel de groupe multi-participants (mesh WebRTC).
  Chaque participant maintient une RTCPeerConnection par pair.
  Affiche une grille de vidéos/avatars pour tous les participants.
-->
<script>
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import { auth } from '../lib/stores.js';
  import { getGroup } from '../lib/api.js';

  /** groupId du groupe, video: boolean, sendWs: function */
  export let groupId;
  export let video = false;
  export let sendWs = () => {};
  /** Messages entrants liés à l'appel de groupe (réactif) */
  export let incomingSignal = null;

  const dispatch = createEventDispatcher();
  const ICE_SERVERS = [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' },
  ];

  let localStream = null;
  let localVideo;
  let audioEnabled = true;
  let videoEnabled = false;
  let duration = 0;
  let durationInterval;
  let groupName = '';

  // Map<userId, { pc: RTCPeerConnection, stream: MediaStream, username: string, videoEl: HTMLVideoElement }>
  let peers = new Map();
  // Reactive array for rendering
  let peerList = [];

  $: if (incomingSignal) handleIncomingSignal(incomingSignal);

  onMount(async () => {
    videoEnabled = video;
    try {
      // Charger les infos du groupe
      const res = await getGroup(groupId);
      groupName = res.group?.name || 'Groupe';

      // Acquérir le flux local
      localStream = await navigator.mediaDevices.getUserMedia({
        audio: true,
        video: videoEnabled,
      });
      if (localVideo) localVideo.srcObject = localStream;

      // Notifier les membres du groupe
      sendWs({ type: 'group_call_start', group_id: groupId, video: videoEnabled, username: $auth.user?.username || '' });

      startDurationTimer();
    } catch (err) {
      console.error('Erreur initialisation appel de groupe:', err);
      hangup();
    }
  });

  onDestroy(() => {
    cleanup();
  });

  /** Crée une connexion pair vers un utilisateur distant */
  function createPeerConnection(userId, username) {
    if (peers.has(userId)) return peers.get(userId).pc;

    const pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });
    const remoteStream = new MediaStream();

    // Ajouter les pistes locales
    if (localStream) {
      localStream.getTracks().forEach(track => pc.addTrack(track, localStream));
    }

    // Réception des pistes distantes
    pc.ontrack = (e) => {
      e.streams[0].getTracks().forEach(track => remoteStream.addTrack(track));
      updatePeerList();
    };

    // Envoi des candidats ICE
    pc.onicecandidate = (e) => {
      if (e.candidate) {
        sendWs({
          type: 'group_ice_candidate',
          target_id: userId,
          group_id: groupId,
          candidate: e.candidate,
        });
      }
    };

    pc.onconnectionstatechange = () => {
      if (['disconnected', 'failed', 'closed'].includes(pc.connectionState)) {
        removePeer(userId);
      }
    };

    peers.set(userId, { pc, stream: remoteStream, username });
    updatePeerList();
    return pc;
  }

  /** Envoyer une offre SDP à un pair */
  async function sendOfferTo(userId, username) {
    const pc = createPeerConnection(userId, username);
    try {
      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);
      sendWs({
        type: 'group_call_offer',
        target_id: userId,
        group_id: groupId,
        offer: pc.localDescription,
        video: videoEnabled,
        username: $auth.user?.username || '',
      });
    } catch (e) {
      console.error('Erreur envoi offer à', userId, e);
    }
  }

  /** Traiter les messages de signalisation entrants */
  async function handleIncomingSignal(signal) {
    if (!signal) return;
    const { type, from_id, username } = signal;

    switch (type) {
      case 'group_call_start':
      case 'group_call_join': {
        // Un nouveau participant rejoint : lui envoyer une offre
        const name = signal.username || username || `User ${from_id}`;
        await sendOfferTo(from_id, name);
        break;
      }
      case 'group_call_offer': {
        // Recevoir une offre : créer la connexion, répondre
        const name = signal.username || `User ${from_id}`;
        const pc = createPeerConnection(from_id, name);
        try {
          await pc.setRemoteDescription(new RTCSessionDescription(signal.offer));
          const answer = await pc.createAnswer();
          await pc.setLocalDescription(answer);
          sendWs({
            type: 'group_call_answer',
            target_id: from_id,
            group_id: groupId,
            answer: pc.localDescription,
            username: $auth.user?.username || '',
          });
        } catch (e) {
          console.error('Erreur réponse offer de', from_id, e);
        }
        break;
      }
      case 'group_call_answer': {
        // Recevoir une réponse à notre offre
        const peer = peers.get(from_id);
        if (peer?.pc && peer.pc.signalingState === 'have-local-offer') {
          try {
            await peer.pc.setRemoteDescription(new RTCSessionDescription(signal.answer));
          } catch (e) {
            console.error('Erreur setRemoteDescription de', from_id, e);
          }
        }
        break;
      }
      case 'group_ice_candidate': {
        const peer = peers.get(from_id);
        if (peer?.pc && signal.candidate) {
          try {
            await peer.pc.addIceCandidate(new RTCIceCandidate(signal.candidate));
          } catch (e) {
            console.error('Erreur ICE de', from_id, e);
          }
        }
        break;
      }
      case 'group_call_leave':
      case 'group_call_hangup': {
        removePeer(from_id);
        break;
      }
    }
  }

  function removePeer(userId) {
    const peer = peers.get(userId);
    if (peer) {
      peer.pc.close();
      peers.delete(userId);
      updatePeerList();
    }
  }

  function updatePeerList() {
    peerList = Array.from(peers.entries()).map(([id, p]) => ({
      id,
      username: p.username,
      stream: p.stream,
    }));
  }

  function hangup() {
    sendWs({ type: 'group_call_leave', group_id: groupId });
    cleanup();
    dispatch('end');
  }

  function toggleAudio() {
    audioEnabled = !audioEnabled;
    localStream?.getAudioTracks().forEach(t => { t.enabled = audioEnabled; });
  }

  function toggleVideo() {
    videoEnabled = !videoEnabled;
    localStream?.getVideoTracks().forEach(t => { t.enabled = videoEnabled; });
  }

  function startDurationTimer() {
    duration = 0;
    durationInterval = setInterval(() => { duration += 1; }, 1000);
  }

  function formatDuration(s) {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
  }

  function cleanup() {
    clearInterval(durationInterval);
    localStream?.getTracks().forEach(t => t.stop());
    for (const [, peer] of peers) {
      peer.pc.close();
    }
    peers.clear();
    peerList = [];
    localStream = null;
  }

  /** Svelte action : lie un élément media à un MediaStream */
  function bindStream(el, stream) {
    if (stream) el.srcObject = stream;
    return {
      update(newStream) {
        if (newStream) el.srcObject = newStream;
      }
    };
  }
</script>

<!-- Overlay plein écran -->
<div class="fixed inset-0 z-50 bg-slate-900/95 backdrop-blur-lg flex flex-col">
  <!-- Header -->
  <div class="p-4 flex items-center justify-between border-b border-slate-700/50">
    <div>
      <h2 class="text-white font-semibold text-lg">{groupName}</h2>
      <p class="text-slate-400 text-sm">
        Appel de groupe · {peerList.length + 1} participant{peerList.length > 0 ? 's' : ''} · {formatDuration(duration)}
      </p>
    </div>
    <button on:click={hangup} class="px-4 py-2 bg-red-500 hover:bg-red-600 text-white rounded-xl font-medium transition-colors">
      Quitter
    </button>
  </div>

  <!-- Grille des participants -->
  <div class="flex-1 p-4 overflow-auto">
    <div class="grid gap-3 h-full"
         style="grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));">
      <!-- Soi-même -->
      <div class="relative bg-slate-800 rounded-2xl overflow-hidden border border-slate-700/50 flex items-center justify-center min-h-[180px]">
        {#if videoEnabled}
          <video bind:this={localVideo} autoplay muted playsinline class="w-full h-full object-cover"></video>
        {:else}
          <div class="flex flex-col items-center gap-2">
            <div class="w-16 h-16 rounded-full bg-primary-500/20 flex items-center justify-center">
              <span class="text-2xl text-primary-400 font-bold">{($auth.user?.username || '?')[0].toUpperCase()}</span>
            </div>
          </div>
        {/if}
        <div class="absolute bottom-2 left-2 px-2 py-1 bg-black/60 rounded-lg text-xs text-white">
          Vous {#if !audioEnabled}<span class="text-red-400">🔇</span>{/if}
        </div>
      </div>

      <!-- Pairs distants -->
      {#each peerList as peer (peer.id)}
        <div class="relative bg-slate-800 rounded-2xl overflow-hidden border border-slate-700/50 flex items-center justify-center min-h-[180px]">
          {#if videoEnabled && peer.stream.getVideoTracks().length > 0}
            <video autoplay playsinline class="w-full h-full object-cover"
                   use:bindStream={peer.stream}></video>
          {:else}
            <div class="flex flex-col items-center gap-2">
              <div class="w-16 h-16 rounded-full bg-emerald-500/20 flex items-center justify-center">
                <span class="text-2xl text-emerald-400 font-bold">{(peer.username || '?')[0].toUpperCase()}</span>
              </div>
            </div>
          {/if}
          <!-- Audio caché pour le son -->
          <audio autoplay use:bindStream={peer.stream} class="hidden"></audio>
          <div class="absolute bottom-2 left-2 px-2 py-1 bg-black/60 rounded-lg text-xs text-white">
            {peer.username}
          </div>
        </div>
      {/each}
    </div>
  </div>

  <!-- Contrôles en bas -->
  <div class="p-4 border-t border-slate-700/50 flex items-center justify-center gap-4">
    <button on:click={toggleAudio}
            class="w-14 h-14 rounded-full {audioEnabled ? 'bg-slate-700 hover:bg-slate-600' : 'bg-red-500/20 hover:bg-red-500/30'} flex items-center justify-center text-white transition-colors"
            title="{audioEnabled ? 'Couper le micro' : 'Activer le micro'}">
      {#if audioEnabled}
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4M12 15a3 3 0 003-3V5a3 3 0 00-6 0v7a3 3 0 003 3z"/></svg>
      {:else}
        <svg class="w-6 h-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z"/><path d="M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2"/></svg>
      {/if}
    </button>

    <button on:click={toggleVideo}
            class="w-14 h-14 rounded-full {videoEnabled ? 'bg-slate-700 hover:bg-slate-600' : 'bg-red-500/20 hover:bg-red-500/30'} flex items-center justify-center text-white transition-colors"
            title="{videoEnabled ? 'Couper la caméra' : 'Activer la caméra'}">
      {#if videoEnabled}
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"/></svg>
      {:else}
        <svg class="w-6 h-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728L5.636 5.636"/></svg>
      {/if}
    </button>

    <button on:click={hangup}
            class="w-16 h-16 rounded-full bg-red-500 hover:bg-red-600 flex items-center justify-center text-white shadow-lg transition-colors"
            title="Quitter l'appel">
      <svg class="w-7 h-7 rotate-[135deg]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path d="M3 5a2 2 0 012-2h3.28a1 1 0 01.948.684l1.498 4.493a1 1 0 01-.502 1.21l-2.257 1.13a11.042 11.042 0 005.516 5.516l1.13-2.257a1 1 0 011.21-.502l4.493 1.498a1 1 0 01.684.949V19a2 2 0 01-2 2h-1C9.716 21 3 14.284 3 6V5z"/>
      </svg>
    </button>
  </div>
</div>
