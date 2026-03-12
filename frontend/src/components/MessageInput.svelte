<!--
  MessageInput — Champ de saisie avec upload de fichiers.
  Auto-resize du textarea, preview de l'image avant envoi.
-->
<script>
  import { createEventDispatcher } from 'svelte';
  import * as api from '../lib/api.js';

  const dispatch = createEventDispatcher();
  let text = '';
  let fileInput;
  let previewUrl = null;
  let previewFile = null;
  let uploading = false;
  let sending = false;
  let textareaEl;

  async function handleSend() {
    if (sending) return;
    const hasText = text.trim().length > 0;
    const hasFile = previewFile !== null;
    if (!hasText && !hasFile) return;

    sending = true;
    try {
      let imageUrl = null;
      let messageType = 'text';

      // Upload de l'image si présente
      if (hasFile) {
        uploading = true;
        const result = await api.uploadImage(previewFile);
        imageUrl = result.url;
        messageType = 'image';
        uploading = false;
      }

      dispatch('send', {
        content: hasText ? text.trim() : (imageUrl || ''),
        messageType,
        imageUrl,
      });

      text = '';
      clearPreview();
      if (textareaEl) { textareaEl.style.height = 'auto'; }
    } catch (e) {
      console.error('Send error:', e);
    } finally {
      uploading = false;
      sending = false;
    }
  }

  function handleFileSelect(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!file.type.startsWith('image/') && !file.type.startsWith('application/')) {
      alert('Type de fichier non supporté');
      return;
    }
    previewFile = file;
    if (file.type.startsWith('image/')) {
      const reader = new FileReader();
      reader.onload = (ev) => { previewUrl = ev.target.result; };
      reader.readAsDataURL(file);
    } else {
      previewUrl = null;
    }
  }

  function clearPreview() {
    previewUrl = null;
    previewFile = null;
    if (fileInput) fileInput.value = '';
  }

  function handleKeydown(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  function autoResize() {
    if (!textareaEl) return;
    textareaEl.style.height = 'auto';
    textareaEl.style.height = Math.min(textareaEl.scrollHeight, 120) + 'px';
  }
</script>

<div class="border-t border-slate-700/50 bg-slate-800/90 backdrop-blur px-4 md:px-6 py-3">
  <!-- Preview de l'image avant envoi -->
  {#if previewUrl || previewFile}
    <div class="mb-3 flex items-start gap-3 animate-slide-up">
      {#if previewUrl}
        <img src={previewUrl} alt="Preview" class="w-20 h-20 rounded-lg object-cover border border-slate-600" />
      {:else}
        <div class="w-20 h-20 rounded-lg bg-slate-700 flex items-center justify-center border border-slate-600">
          <svg class="w-8 h-8 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>
        </div>
      {/if}
      <div class="flex-1 min-w-0">
        <p class="text-sm text-slate-300 truncate">{previewFile?.name}</p>
        <p class="text-xs text-slate-500">{(previewFile?.size / 1024).toFixed(1)} Ko</p>
      </div>
      <button on:click={clearPreview} class="text-slate-400 hover:text-red-400 p-1 rounded transition-colors">
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M6 18L18 6M6 6l12 12"/></svg>
      </button>
    </div>
  {/if}

  <div class="flex items-end gap-2">
    <!-- Bouton pièce jointe -->
    <button on:click={() => fileInput.click()} class="flex-shrink-0 p-2.5 text-slate-400 hover:text-primary-400 hover:bg-slate-700/50 rounded-xl transition-colors">
      <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/>
      </svg>
    </button>
    <input bind:this={fileInput} type="file" accept="image/*,.pdf,.doc,.docx,.txt" on:change={handleFileSelect} class="hidden" />

    <!-- Textarea -->
    <textarea
      bind:this={textareaEl}
      bind:value={text}
      on:keydown={handleKeydown}
      on:input={autoResize}
      placeholder="Écrire un message..."
      rows="1"
      class="flex-1 resize-none bg-slate-700/50 border border-slate-600/50 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-400
             focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all max-h-[120px]"
    ></textarea>

    <!-- Bouton envoyer -->
    <button
      on:click={handleSend}
      disabled={(!text.trim() && !previewFile) || sending}
      class="flex-shrink-0 p-2.5 bg-primary-500 hover:bg-primary-600 disabled:bg-slate-700 disabled:text-slate-500
             text-white rounded-xl transition-all shadow-lg shadow-primary-500/20 hover:shadow-primary-500/30
             disabled:shadow-none disabled:cursor-not-allowed"
    >
      {#if uploading || sending}
        <svg class="w-5 h-5 animate-spin" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"/><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/></svg>
      {:else}
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path d="M12 19V5M5 12l7-7 7 7"/></svg>
      {/if}
    </button>
  </div>
</div>
