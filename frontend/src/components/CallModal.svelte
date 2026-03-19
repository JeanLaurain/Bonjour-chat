<!--
  CallModal — Interface d'appel audio/vidéo WebRTC.
  Gère la négociation SDP (offer/answer), les candidats ICE,
  les flux média (getUserMedia) et l'UI d'appel.
-->
<script>
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';

  /** État de l'appel reçu du parent (outgoing/incoming, userId, username, video, offer) */
  export let callState = null;
  /** Fonction pour envoyer un message via WebSocket */
  export let sendWs = () => {};

  const dispatch = createEventDispatcher();

  // Serveurs STUN publics pour la résolution NAT
  const ICE_SERVERS = [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' },
  ];

  let pc = null; // RTCPeerConnection
  let localStream = null;
  let remoteStream = null;
  let localVideo;
  let remoteVideo;
  let status = 'initializing'; // initializing, ringing, connected, ended
  let duration = 0;
  let durationInterval;
  let audioEnabled = true;
  let videoEnabled = false;
  let ringtone;

  $: if (callState?.answer) handleRemoteAnswer(callState.answer);
  $: if (callState?.newIceCandidate) handleRemoteIceCandidate(callState.newIceCandidate);

  onMount(async () => {
    videoEnabled = callState?.video || false;
    if (callState?.type === 'outgoing') {
      await startOutgoingCall();
    } else if (callState?.type === 'incoming') {
      status = 'ringing';
      playRingtone();
    }
  });

  onDestroy(() => {
    cleanup();
  });

  /** Initialise le flux local et la connexion peer */
  async function initPeerConnection() {
    try {
      localStream = await navigator.mediaDevices.getUserMedia({
        audio: true,
        video: videoEnabled,
      });

      pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });

      // Ajouter les pistes locales
      localStream.getTracks().forEach(track => pc.addTrack(track, localStream));

      // Réception des pistes distantes
      remoteStream = new MediaStream();
      pc.ontrack = (e) => {
        e.streams[0].getTracks().forEach(track => remoteStream.addTrack(track));
        if (remoteVideo) remoteVideo.srcObject = remoteStream;
      };

      // Envoi des candidats ICE au pair distant
      pc.onicecandidate = (e) => {
        if (e.candidate) {
          sendWs({
            type: 'ice_candidate',
            target_id: callState.userId,
            candidate: e.candidate,
          });
        }
      };

      // Surveillance de l'état de la connexion
      pc.onconnectionstatechange = () => {
        if (pc.connectionState === 'connected') {
          status = 'connected';
          dispatch('connected');
          startDurationTimer();
        }
        if (['disconnected', 'failed', 'closed'].includes(pc.connectionState)) {
          hangup();
        }
      };

      // Afficher la vidéo locale
      if (localVideo) localVideo.srcObject = localStream;
    } catch (err) {
      console.error('Erreur accès média:', err);
      hangup();
    }
  }

  /** Appel sortant : créer offer et envoyer */
  async function startOutgoingCall() {
    status = 'ringing';
    await initPeerConnection();
    if (!pc) return;

    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);

    sendWs({
      type: 'call_offer',
      target_id: callState.userId,
      offer: pc.localDescription,
      video: videoEnabled,
    });
  }

  /** Accepter un appel entrant */
  async function acceptCall() {
    stopRingtone();
    await initPeerConnection();
    if (!pc || !callState?.offer) return;

    await pc.setRemoteDescription(new RTCSessionDescription(callState.offer));
    const answer = await pc.createAnswer();
    await pc.setLocalDescription(answer);

    sendWs({
      type: 'call_answer',
      target_id: callState.userId,
      answer: pc.localDescription,
    });

    status = 'connected';
    dispatch('connected');
    startDurationTimer();
  }

  /** Refuser un appel entrant */
  function rejectCall() {
    stopRingtone();
    sendWs({ type: 'call_reject', target_id: callState.userId });
    cleanup();
    dispatch('end');
  }

  /** Traiter la réponse SDP distante (appel sortant) */
  async function handleRemoteAnswer(answer) {
    if (!pc || pc.signalingState !== 'have-local-offer') return;
    try {
      await pc.setRemoteDescription(new RTCSessionDescription(answer));
    } catch (e) { console.error('Erreur setRemoteDescription:', e); }
  }

  /** Ajouter un candidat ICE distant */
  async function handleRemoteIceCandidate(candidate) {
    if (!pc) return;
    try {
      await pc.addIceCandidate(new RTCIceCandidate(candidate));
    } catch (e) { console.error('Erreur addIceCandidate:', e); }
  }

  /** Raccrocher / terminer l'appel */
  function hangup() {
    sendWs({ type: 'call_hangup', target_id: callState?.userId });
    cleanup();
    dispatch('end');
  }

  /** Toggle audio on/off */
  function toggleAudio() {
    audioEnabled = !audioEnabled;
    localStream?.getAudioTracks().forEach(t => { t.enabled = audioEnabled; });
  }

  /** Toggle vidéo on/off */
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

  /** Jouer une sonnerie (ton simple via Web Audio API) */
  function playRingtone() {
    try {
      const ctx = new (window.AudioContext || window.webkitAudioContext)();
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = 440;
      gain.gain.value = 0.2;
      osc.start();
      ringtone = { ctx, osc, gain };
      // Alterner la sonnerie
      const toggle = setInterval(() => {
        if (!ringtone) { clearInterval(toggle); return; }
        gain.gain.value = gain.gain.value > 0 ? 0 : 0.2;
      }, 1000);
      ringtone.toggleInterval = toggle;
    } catch {}
  }

  function stopRingtone() {
    if (ringtone) {
      clearInterval(ringtone.toggleInterval);
      ringtone.osc.stop();
      ringtone.ctx.close();
      ringtone = null;
    }
  }

  /** Libérer toutes les ressources */
  function cleanup() {
    stopRingtone();
    clearInterval(durationInterval);
    localStream?.getTracks().forEach(t => t.stop());
    pc?.close();
    pc = null;
    localStream = null;
    remoteStream = null;
    status = 'ended';
  }
</script>

<!-- Overlay plein écran pour l'appel -->
<div class="fixed inset-0 z-50 bg-slate-900/95 backdrop-blur-lg flex flex-col items-center justify-center">
  <!-- Info de l'appel -->
  <div class="text-center mb-8">
    <!-- Avatar -->
    <div class="w-24 h-24 rounded-full bg-primary-500/20 flex items-center justify-center mx-auto mb-4 {status === 'ringing' ? 'animate-pulse' : ''}">
      <span class="text-4xl text-primary-400 font-bold">
        {(callState?.username || '?')[0].toUpperCase()}
      </span>
    </div>
    <h2 class="text-xl font-semibold text-white">{callState?.username || ''}</h2>
    <p class="text-sm text-slate-400 mt-1">
      {#if status === 'ringing' && callState?.type === 'outgoing'}
        Appel en cours...
      {:else if status === 'ringing' && callState?.type === 'incoming'}
        Appel {callState?.video ? 'vidéo' : 'audio'} entrant
      {:else if status === 'connected'}
        {formatDuration(duration)}
      {:else if status === 'ended'}
        Appel terminé
      {:else}
        Connexion...
      {/if}
    </p>
  </div>

  <!-- Zone vidéo (si appel vidéo et connecté) -->
  {#if videoEnabled && status === 'connected'}
    <div class="relative w-full max-w-2xl aspect-video mb-8 rounded-2xl overflow-hidden bg-black">
      <!-- Vidéo distante (plein cadre) -->
      <video bind:this={remoteVideo} autoplay playsinline class="w-full h-full object-cover"></video>
      <!-- Vidéo locale (miniature en bas à droite) -->
      <video bind:this={localVideo} autoplay muted playsinline class="absolute bottom-3 right-3 w-32 h-24 rounded-lg object-cover border-2 border-white/20 shadow-lg"></video>
    </div>
  {:else}
    <!-- Audio uniquement : vidéo locale cachée -->
    <video bind:this={localVideo} autoplay muted playsinline class="hidden"></video>
    <video bind:this={remoteVideo} autoplay playsinline class="hidden"></video>
  {/if}

  <!-- Boutons de contrôle -->
  <div class="flex items-center gap-4">
    {#if status === 'ringing' && callState?.type === 'incoming'}
      <!-- Appel entrant : Accepter / Refuser -->
      <button on:click={rejectCall} class="w-16 h-16 rounded-full bg-red-500 hover:bg-red-600 flex items-center justify-center text-white shadow-lg transition-colors" title="Refuser">
        <svg class="w-7 h-7" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path d="M6 18L18 6M6 6l12 12"/>
        </svg>
      </button>
      <button on:click={acceptCall} class="w-16 h-16 rounded-full bg-emerald-500 hover:bg-emerald-600 flex items-center justify-center text-white shadow-lg transition-colors animate-bounce" title="Accepter">
        <svg class="w-7 h-7" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path d="M3 5a2 2 0 012-2h3.28a1 1 0 01.948.684l1.498 4.493a1 1 0 01-.502 1.21l-2.257 1.13a11.042 11.042 0 005.516 5.516l1.13-2.257a1 1 0 011.21-.502l4.493 1.498a1 1 0 01.684.949V19a2 2 0 01-2 2h-1C9.716 21 3 14.284 3 6V5z"/>
        </svg>
      </button>
    {:else}
      <!-- En appel ou en attente : contrôles -->
      <button on:click={toggleAudio} class="w-14 h-14 rounded-full {audioEnabled ? 'bg-slate-700 hover:bg-slate-600' : 'bg-red-500/20 hover:bg-red-500/30'} flex items-center justify-center text-white transition-colors" title="{audioEnabled ? 'Couper le micro' : 'Activer le micro'}">
        {#if audioEnabled}
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4M12 15a3 3 0 003-3V5a3 3 0 00-6 0v7a3 3 0 003 3z"/></svg>
        {:else}
          <svg class="w-6 h-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z"/><path d="M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2"/></svg>
        {/if}
      </button>

      {#if callState?.video || videoEnabled}
        <button on:click={toggleVideo} class="w-14 h-14 rounded-full {videoEnabled ? 'bg-slate-700 hover:bg-slate-600' : 'bg-red-500/20 hover:bg-red-500/30'} flex items-center justify-center text-white transition-colors" title="{videoEnabled ? 'Couper la caméra' : 'Activer la caméra'}">
          {#if videoEnabled}
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"/></svg>
          {:else}
            <svg class="w-6 h-6 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728L5.636 5.636m12.728 12.728L5.636 5.636"/></svg>
          {/if}
        </button>
      {/if}

      <button on:click={hangup} class="w-16 h-16 rounded-full bg-red-500 hover:bg-red-600 flex items-center justify-center text-white shadow-lg transition-colors" title="Raccrocher">
        <svg class="w-7 h-7 rotate-[135deg]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path d="M3 5a2 2 0 012-2h3.28a1 1 0 01.948.684l1.498 4.493a1 1 0 01-.502 1.21l-2.257 1.13a11.042 11.042 0 005.516 5.516l1.13-2.257a1 1 0 011.21-.502l4.493 1.498a1 1 0 01.684.949V19a2 2 0 01-2 2h-1C9.716 21 3 14.284 3 6V5z"/>
        </svg>
      </button>
    {/if}
  </div>
</div>
