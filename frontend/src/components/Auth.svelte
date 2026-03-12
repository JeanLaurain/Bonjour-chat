<!--
  Auth — Page de connexion / inscription avec design moderne.
  Toggle entre login et register via un onglet.
-->
<script>
  import { auth } from '../lib/stores.js';
  import * as api from '../lib/api.js';

  let mode = 'login'; // 'login' | 'register'
  let username = '';
  let email = '';
  let password = '';
  let error = '';
  let loading = false;

  async function handleSubmit() {
    error = '';
    loading = true;
    try {
      if (mode === 'register') {
        const data = await api.register(username, email, password);
        auth.login(data.token, data.user);
      } else {
        const data = await api.login(username, password);
        auth.login(data.token, data.user);
      }
    } catch (e) {
      error = e.message || 'Une erreur est survenue';
    } finally {
      loading = false;
    }
  }

  function toggleMode() {
    mode = mode === 'login' ? 'register' : 'login';
    error = '';
  }
</script>

<div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-slate-900 via-primary-900 to-slate-900 p-4">
  <!-- Cercles décoratifs d'arrière-plan -->
  <div class="absolute inset-0 overflow-hidden pointer-events-none">
    <div class="absolute -top-40 -left-40 w-96 h-96 bg-primary-500/10 rounded-full blur-3xl"></div>
    <div class="absolute -bottom-40 -right-40 w-96 h-96 bg-primary-600/10 rounded-full blur-3xl"></div>
  </div>

  <div class="relative w-full max-w-md animate-fade-in">
    <!-- Logo -->
    <div class="text-center mb-8">
      <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-primary-500 text-white text-3xl mb-4 shadow-lg shadow-primary-500/30">
        💬
      </div>
      <h1 class="text-3xl font-bold text-white">Bonjour</h1>
      <p class="text-slate-400 mt-1">Messagerie instantanée sécurisée</p>
    </div>

    <!-- Card -->
    <div class="bg-slate-800/80 backdrop-blur-xl rounded-2xl shadow-2xl border border-slate-700/50 p-8">
      <!-- Onglets Login / Register -->
      <div class="flex rounded-xl bg-slate-900/50 p-1 mb-6">
        <button
          class="flex-1 py-2.5 text-sm font-medium rounded-lg transition-all duration-200 {mode === 'login' ? 'bg-primary-500 text-white shadow-lg' : 'text-slate-400 hover:text-white'}"
          on:click={() => { mode = 'login'; error = ''; }}
        >
          Connexion
        </button>
        <button
          class="flex-1 py-2.5 text-sm font-medium rounded-lg transition-all duration-200 {mode === 'register' ? 'bg-primary-500 text-white shadow-lg' : 'text-slate-400 hover:text-white'}"
          on:click={() => { mode = 'register'; error = ''; }}
        >
          Inscription
        </button>
      </div>

      <!-- Formulaire -->
      <form on:submit|preventDefault={handleSubmit} class="space-y-4">
        <div>
          <label for="username" class="block text-sm font-medium text-slate-300 mb-1.5">
            Nom d'utilisateur
          </label>
          <input
            id="username"
            type="text"
            bind:value={username}
            placeholder="alice"
            required
            minlength="3"
            class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500
                   focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
          />
        </div>

        {#if mode === 'register'}
          <div class="animate-slide-up">
            <label for="email" class="block text-sm font-medium text-slate-300 mb-1.5">
              Email
            </label>
            <input
              id="email"
              type="email"
              bind:value={email}
              placeholder="alice@example.com"
              required
              class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500
                     focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
            />
          </div>
        {/if}

        <div>
          <label for="password" class="block text-sm font-medium text-slate-300 mb-1.5">
            Mot de passe
          </label>
          <input
            id="password"
            type="password"
            bind:value={password}
            placeholder="••••••"
            required
            minlength="6"
            class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500
                   focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
          />
        </div>

        {#if error}
          <div class="bg-red-500/10 border border-red-500/30 text-red-400 text-sm rounded-xl px-4 py-3 animate-fade-in">
            {error}
          </div>
        {/if}

        <button
          type="submit"
          disabled={loading}
          class="w-full py-3 bg-primary-500 hover:bg-primary-600 disabled:bg-primary-500/50 text-white font-semibold
                 rounded-xl transition-all duration-200 shadow-lg shadow-primary-500/25 hover:shadow-primary-500/40
                 disabled:cursor-not-allowed"
        >
          {#if loading}
            <span class="inline-flex items-center gap-2">
              <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none"/><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/></svg>
              Chargement...
            </span>
          {:else}
            {mode === 'login' ? 'Se connecter' : "S'inscrire"}
          {/if}
        </button>
      </form>

      <!-- Toggle link -->
      <p class="text-center text-slate-400 text-sm mt-6">
        {mode === 'login' ? 'Pas encore de compte ?' : 'Déjà un compte ?'}
        <button on:click={toggleMode} class="text-primary-400 hover:text-primary-300 font-medium ml-1">
          {mode === 'login' ? "S'inscrire" : 'Se connecter'}
        </button>
      </p>
    </div>
  </div>
</div>
