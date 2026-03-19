<!--
  ProfileModal — Modale de profil utilisateur.
  Permet de voir et modifier la photo de profil.
  La photo est uploadée via /upload puis l'URL est sauvegardée via /auth/profile.
-->
<script>
  import { createEventDispatcher } from 'svelte';
  import { auth } from '../lib/stores.js';
  import * as api from '../lib/api.js';

  const dispatch = createEventDispatcher();
  let uploading = false;
  let error = '';
  let fileInput;

  $: user = $auth.user;

  /** Upload une photo et met à jour le profil */
  async function handleFileSelect(e) {
    const file = e.target.files?.[0];
    if (!file) return;

    // Vérifier que c'est une image
    if (!file.type.startsWith('image/')) {
      error = 'Veuillez sélectionner une image';
      return;
    }

    uploading = true;
    error = '';
    try {
      // 1. Upload le fichier
      const uploadResult = await api.uploadImage(file);
      // 2. Mettre à jour le profil avec l'URL
      const profileResult = await api.updateProfile(uploadResult.url);
      // 3. Mettre à jour le store local
      auth.updateUser(profileResult.user);
    } catch (e) {
      error = e.message || 'Erreur lors de la mise à jour';
    } finally {
      uploading = false;
      if (fileInput) fileInput.value = '';
    }
  }

  /** Supprimer la photo de profil */
  async function removePhoto() {
    uploading = true;
    error = '';
    try {
      const result = await api.updateProfile(null);
      auth.updateUser(result.user);
    } catch (e) {
      error = e.message || 'Erreur lors de la suppression';
    } finally {
      uploading = false;
    }
  }

  function close() {
    dispatch('close');
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') close();
  }

  /** Génère les initiales pour l'avatar par défaut */
  function getInitials(name) {
    if (!name) return '?';
    return name.split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase();
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- Overlay -->
<div class="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 animate-fade-in"
     on:click|self={close} role="dialog" aria-modal="true">
  <div class="bg-slate-800 rounded-2xl shadow-2xl border border-slate-700/50 w-full max-w-sm animate-slide-up">
    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-4 border-b border-slate-700/50">
      <h2 class="text-lg font-semibold text-white">Mon profil</h2>
      <button on:click={close} class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-700/50 transition-colors">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
      </button>
    </div>

    <!-- Corps -->
    <div class="px-5 py-6 flex flex-col items-center gap-5">
      <!-- Avatar actuel -->
      <div class="relative group">
        {#if user?.profile_picture_url}
          <img src={user.profile_picture_url} alt="Photo de profil"
               class="w-28 h-28 rounded-full object-cover border-4 border-slate-600 shadow-lg" />
        {:else}
          <div class="w-28 h-28 rounded-full bg-primary-500 flex items-center justify-center text-4xl text-white font-bold border-4 border-slate-600 shadow-lg">
            {getInitials(user?.username)}
          </div>
        {/if}

        <!-- Overlay au survol pour changer la photo -->
        <button
          on:click={() => fileInput.click()}
          class="absolute inset-0 rounded-full bg-black/50 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity cursor-pointer"
          disabled={uploading}
        >
          {#if uploading}
            <svg class="w-8 h-8 text-white animate-spin" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"/>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
            </svg>
          {:else}
            <svg class="w-8 h-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z"/>
              <circle cx="12" cy="13" r="3"/>
            </svg>
          {/if}
        </button>
      </div>

      <input bind:this={fileInput} type="file" accept="image/*" on:change={handleFileSelect} class="hidden" />

      <!-- Nom et email -->
      <div class="text-center">
        <h3 class="text-lg font-semibold text-white">{user?.username || ''}</h3>
        <p class="text-sm text-slate-400">{user?.email || ''}</p>
      </div>

      {#if error}
        <p class="text-sm text-red-400">{error}</p>
      {/if}

      <!-- Actions -->
      <div class="flex gap-3 w-full">
        <button
          on:click={() => fileInput.click()}
          disabled={uploading}
          class="flex-1 px-4 py-2.5 bg-primary-500 hover:bg-primary-600 disabled:bg-slate-700 text-white text-sm font-medium rounded-xl transition-colors"
        >
          {uploading ? 'Envoi...' : 'Changer la photo'}
        </button>
        {#if user?.profile_picture_url}
          <button
            on:click={removePhoto}
            disabled={uploading}
            class="px-4 py-2.5 bg-red-500/10 hover:bg-red-500/20 text-red-400 text-sm font-medium rounded-xl transition-colors"
          >
            Supprimer
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>
