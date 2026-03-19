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

  // Serveurs STUN/TURN pour la résolution NAT
  const ICE_SERVERS = [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' },
    { urls: 'stun:stun2.l.google.com:19302' },
    { urls: 'stun:stun3.l.google.com:19302' },
    { urls: 'stun:stun4.l.google.com:19302' },
  ];

  let pc = null; // RTCPeerConnection
  let localStream = null;
  let remoteStream = null;
  let localVideo;
  let remoteVideo;
  let remoteAudio;
  let status = 'initializing'; // initializing, ringing, connecting, connected, ended
  let duration = 0;
  let durationInterval;
  let audioEnabled = true;
  let videoEnabled = false;
  let ringtone;
  let debugInfo = ''; // Info de debug visible

  // Buffer de candidats ICE reçus avant setRemoteDescription
  let candidateBuffer = [];
  let remoteDescSet = false;
  let processedCandidates = 0;
  let answerProcessed = false; // Éviter de traiter l'answer plusieurs fois

  // Sérialise un RTCSessionDescription en objet simple {type, sdp}
  function serializeSDP(desc) {
    if (!desc) return null;
    return { type: desc.type, sdp: desc.sdp };
  }

  // Sérialise un RTCIceCandidate en objet simple
  function serializeCandidate(c) {
    if (!c) return null;
    return { candidate: c.candidate, sdpMid: c.sdpMid, sdpMLineIndex: c.sdpMLineIndex, usernameFragment: c.usernameFragment };
  }

  // Réactive : traiter la réponse SDP quand elle arrive (appel sortant)
  $: if (callState?.answer && !answerProcessed) {
    answerProcessed = true;
    handleRemoteAnswer(callState.answer);
  }

  // Réactive : traiter les nouveaux candidats ICE (accumulés dans un array)
  $: if (callState?.iceCandidates?.length > processedCandidates) {
    const newOnes = callState.iceCandidates.slice(processedCandidates);
    processedCandidates = callState.iceCandidates.length;
    newOnes.forEach(c => handleRemoteIceCandidate(c));
  }

  onMount(async () => {
    videoEnabled = callState?.video || false;
    console.log('[Call] onMount, type:', callState?.type, 'video:', videoEnabled);
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
      console.log('[Call] getUserMedia OK, tracks:', localStream.getTracks().map(t => t.kind));

      pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });

      // Ajouter les pistes locales
      localStream.getTracks().forEach(track => pc.addTrack(track, localStream));

      // Réception des pistes distantes (audio/vidéo)
      remoteStream = new MediaStream();
      pc.ontrack = (e) => {
        console.log('[Call] ontrack:', e.track.kind);
        remoteStream.addTrack(e.track);
        // Connecter le flux aux éléments de lecture
        if (remoteVideo) remoteVideo.srcObject = remoteStream;
        if (remoteAudio) remoteAudio.srcObject = remoteStream;
      };

      // Envoi des candidats ICE au pair distant (sérialisation explicite)
      pc.onicecandidate = (e) => {
        if (e.candidate) {
          console.log('[Call] ICE candidate local:', e.candidate.candidate?.substring(0, 50));
          sendWs({
            type: 'ice_candidate',
            target_id: callState.userId,
            candidate: serializeCandidate(e.candidate),
          });
        }
      };

      // Surveillance de l'état ICE
      pc.oniceconnectionstatechange = () => {
        if (!pc) return;
        const s = pc.iceConnectionState;
        console.log('[Call] ICE connection state:', s);
        debugInfo = `ICE: ${s}`;
        if ((s === 'connected' || s === 'completed') && status !== 'connected') {
          status = 'connected';
          dispatch('connected');
          startDurationTimer();
        }
        if (s === 'failed') {
          debugInfo = 'ICE: failed — connexion impossible';
          hangup();
        }
        if (s === 'closed') {
          hangup();
        }
      };

      // Surveillance connectionState (browsers modernes)
      pc.onconnectionstatechange = () => {
        if (!pc) return;
        console.log('[Call] Connection state:', pc.connectionState);
        if (pc.connectionState === 'connected' && status !== 'connected') {
          status = 'connected';
          dispatch('connected');
          startDurationTimer();
        }
        if (['disconnected', 'failed', 'closed'].includes(pc.connectionState)) {
          hangup();
        }
      };

      pc.onicegatheringstatechange = () => {
        if (pc) console.log('[Call] ICE gathering state:', pc.iceGatheringState);
      };

      pc.onsignalingstatechange = () => {
        if (pc) console.log('[Call] Signaling state:', pc.signalingState);
      };

      // Afficher la vidéo locale
      if (localVideo) localVideo.srcObject = localStream;
    } catch (err) {
      console.error('[Call] Erreur accès média:', err);
      debugInfo = 'Erreur: ' + err.message;
      hangup();
    }
  }

  /** Appel sortant : créer offer et envoyer */
  async function startOutgoingCall() {
    status = 'ringing';
    await initPeerConnection();
    if (!pc) return;

    try {
      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);
      console.log('[Call] Offer créée, signaling:', pc.signalingState);

      sendWs({
        type: 'call_offer',
        target_id: callState.userId,
        offer: serializeSDP(pc.localDescription),
        video: videoEnabled,
      });
      console.log('[Call] Offer envoyée à user', callState.userId);
    } catch (err) {
      console.error('[Call] Erreur création offer:', err);
      hangup();
    }
  }

  /** Accepter un appel entrant */
  async function acceptCall() {
    stopRingtone();
    status = 'connecting';
    debugInfo = 'Connexion...';
    await initPeerConnection();
    if (!pc || !callState?.offer) return;

    try {
      console.log('[Call] Setting remote desc (offer)...');
      await pc.setRemoteDescription(new RTCSessionDescription(callState.offer));
      remoteDescSet = true;
      console.log('[Call] Remote desc set, flushing', candidateBuffer.length, 'buffered candidates');
      await flushCandidateBuffer();

      const answer = await pc.createAnswer();
      await pc.setLocalDescription(answer);

      sendWs({
        type: 'call_answer',
        target_id: callState.userId,
        answer: serializeSDP(pc.localDescription),
      });
      console.log('[Call] Answer envoyée à user', callState.userId);

      // Le statut passera à 'connected' via oniceconnectionstatechange
      status = 'connecting';
      debugInfo = 'En attente ICE...';
    } catch (err) {
      console.error('[Call] Erreur acceptCall:', err);
      debugInfo = 'Erreur: ' + err.message;
    }
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
    console.log('[Call] handleRemoteAnswer, pc:', !!pc, 'signalingState:', pc?.signalingState);
    if (!pc || pc.signalingState !== 'have-local-offer') {
      console.warn('[Call] Cannot set answer, state:', pc?.signalingState);
      return;
    }
    try {
      status = 'connecting';
      debugInfo = 'Answer reçue, connexion...';
      await pc.setRemoteDescription(new RTCSessionDescription(answer));
      remoteDescSet = true;
      console.log('[Call] Remote desc (answer) set, flushing', candidateBuffer.length, 'buffered candidates');
      await flushCandidateBuffer();
    } catch (e) {
      console.error('[Call] Erreur setRemoteDescription:', e);
      debugInfo = 'Erreur SDP: ' + e.message;
    }
  }

  /** Ajouter un candidat ICE distant (bufferisé si nécessaire) */
  async function handleRemoteIceCandidate(candidate) {
    if (!pc || !remoteDescSet) {
      candidateBuffer.push(candidate);
      console.log('[Call] ICE candidate buffered (total:', candidateBuffer.length, ')');
      return;
    }
    try {
      await pc.addIceCandidate(new RTCIceCandidate(candidate));
      console.log('[Call] ICE candidate ajouté');
    } catch (e) {
      console.error('[Call] Erreur addIceCandidate:', e);
    }
  }

  /** Appliquer tous les candidats ICE en attente */
  async function flushCandidateBuffer() {
    const buffered = [...candidateBuffer];
    candidateBuffer = [];
    for (const c of buffered) {
      try {
        await pc.addIceCandidate(new RTCIceCandidate(c));
      } catch (e) { console.error('[Call] Erreur flush ICE:', e); }
    }
    console.log('[Call] Flushed', buffered.length, 'candidates');
  }

  /** Raccrocher / terminer l'appel */
  function hangup() {
    sendWs({ type: 'call_hangup', target_id: callState?.userId });
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
    if (durationInterval) return; // Éviter les doublons
    duration = 0;
    durationInterval = setInterval(() => { duration += 1; }, 1000);
  }

  function formatDuration(s) {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
  }

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

  function cleanup() {
    stopRingtone();
    clearInterval(durationInterval);
    durationInterval = null;
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
      {:else if status === 'connecting'}
        Connexion en cours...
      {:else if status === 'connected'}
        {formatDuration(duration)}
      {:else if status === 'ended'}
        Appel terminé
      {:else}
        Initialisation...
      {/if}
    </p>
    {#if debugInfo}
      <p class="text-xs text-slate-500 mt-1">{debugInfo}</p>
    {/if}
  </div>

  <!-- Zone vidéo : éléments stables pour ne pas perdre srcObject -->
  <div class="relative w-full max-w-2xl aspect-video mb-8 rounded-2xl overflow-hidden bg-black {videoEnabled && status === 'connected' ? '' : 'hidden'}">
    <video bind:this={remoteVideo} autoplay playsinline class="w-full h-full object-cover"></video>
    <video bind:this={localVideo} autoplay muted playsinline class="absolute bottom-3 right-3 w-32 h-24 rounded-lg object-cover border-2 border-white/20 shadow-lg"></video>
  </div>
  <!-- Élément audio pour les appels non-vidéo -->
  {#if !(videoEnabled && status === 'connected')}
    <audio bind:this={remoteAudio} autoplay></audio>
  {/if}

  <!-- Boutons de contrôle -->
  <div class="flex items-center gap-4">
    {#if status === 'ringing' && callState?.type === 'incoming'}
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
