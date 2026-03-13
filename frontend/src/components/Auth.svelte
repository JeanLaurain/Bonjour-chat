<!--
  Auth — Page de connexion / inscription / réinitialisation de mot de passe.
  Trois modes : login, register, forgot.
  Après inscription, le code de récupération est affiché une seule fois.
-->
<script>
  import { auth } from '../lib/stores.js';
  import * as api from '../lib/api.js';

  let mode = 'login'; // 'login' | 'register' | 'forgot'
  let username = '';
  let email = '';
  let password = '';
  let newPassword = '';
  let recoveryCode = '';
  let error = '';
  let success = '';
  let loading = false;

  // Code de récupération affiché après inscription
  let showRecoveryCode = false;
  let generatedRecoveryCode = '';

  async function handleSubmit() {
    error = '';
    success = '';
    loading = true;
    try {
      if (mode === 'register') {
        const data = await api.register(username, email, password);
        // Afficher le code de récupération avant de connecter
        if (data.recovery_code) {
          generatedRecoveryCode = data.recovery_code;
          showRecoveryCode = true;
          // Stocker le token pour se connecter après
          window._pendingAuth = { token: data.token, user: data.user };
        } else {
          auth.login(data.token, data.user);
        }
      } else if (mode === 'forgot') {
        await api.resetPassword(username, recoveryCode, newPassword);
        success = 'Mot de passe réinitialisé ! Vous pouvez vous connecter.';
        setTimeout(() => { mode = 'login'; success = ''; }, 2000);
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

  function continueAfterRecovery() {
    showRecoveryCode = false;
    if (window._pendingAuth) {
      auth.login(window._pendingAuth.token, window._pendingAuth.user);
      delete window._pendingAuth;
    }
  }

  function copyRecoveryCode() {
    navigator.clipboard.writeText(generatedRecoveryCode);
    success = 'Code copié !';
    setTimeout(() => success = '', 2000);
  }

  function switchMode(newMode) {
    mode = newMode;
    error = '';
    success = '';
    recoveryCode = '';
  }
</script>

{#if showRecoveryCode}
  <!-- Modal : code de récupération affiché une seule fois après inscription -->
  <div class="min-h-screen flex items-center justify-center bg-gradient-to-br from-slate-900 via-primary-900 to-slate-900 p-4">
    <div class="relative w-full max-w-md animate-fade-in">
      <div class="bg-slate-800/80 backdrop-blur-xl rounded-2xl shadow-2xl border border-slate-700/50 p-8 text-center">
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-amber-500/20 text-amber-400 text-3xl mb-4">
          🔑
        </div>
        <h2 class="text-xl font-bold text-white mb-2">Code de récupération</h2>
        <p class="text-slate-400 text-sm mb-6">
          Notez ce code en lieu sûr. Il est la <strong class="text-amber-400">seule façon</strong> de récupérer votre compte en cas d'oubli de mot de passe. Il ne sera <strong class="text-amber-400">plus jamais affiché</strong>.
        </p>
        <div class="bg-slate-900 rounded-xl p-4 mb-6 flex items-center justify-center gap-3">
          <code class="text-2xl font-mono font-bold text-amber-400 tracking-wider">{generatedRecoveryCode}</code>
          <button on:click={copyRecoveryCode} class="text-slate-400 hover:text-white transition-colors" title="Copier">
            📋
          </button>
        </div>
        {#if success}
          <div class="text-green-400 text-sm mb-4">{success}</div>
        {/if}
        <button
          on:click={continueAfterRecovery}
          class="w-full py-3 bg-primary-500 hover:bg-primary-600 text-white font-semibold rounded-xl transition-all duration-200 shadow-lg"
        >
          J'ai noté mon code, continuer
        </button>
      </div>
    </div>
  </div>
{:else}
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
      {#if mode !== 'forgot'}
        <div class="flex rounded-xl bg-slate-900/50 p-1 mb-6">
          <button
            class="flex-1 py-2.5 text-sm font-medium rounded-lg transition-all duration-200 {mode === 'login' ? 'bg-primary-500 text-white shadow-lg' : 'text-slate-400 hover:text-white'}"
            on:click={() => switchMode('login')}
          >
            Connexion
          </button>
          <button
            class="flex-1 py-2.5 text-sm font-medium rounded-lg transition-all duration-200 {mode === 'register' ? 'bg-primary-500 text-white shadow-lg' : 'text-slate-400 hover:text-white'}"
            on:click={() => switchMode('register')}
          >
            Inscription
          </button>
        </div>
      {:else}
        <div class="mb-6">
          <button
            on:click={() => switchMode('login')}
            class="text-slate-400 hover:text-white text-sm flex items-center gap-1 transition-colors"
          >
            ← Retour à la connexion
          </button>
          <h2 class="text-lg font-semibold text-white mt-3">Réinitialiser le mot de passe</h2>
          <p class="text-slate-400 text-sm mt-1">Entrez votre nom d'utilisateur et le code de récupération reçu lors de votre inscription.</p>
        </div>
      {/if}

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

        {#if mode === 'forgot'}
          <div class="animate-slide-up">
            <label for="recoveryCode" class="block text-sm font-medium text-slate-300 mb-1.5">
              Code de récupération
            </label>
            <input
              id="recoveryCode"
              type="text"
              bind:value={recoveryCode}
              placeholder="ABCD1234"
              required
              minlength="8"
              maxlength="8"
              class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500 font-mono tracking-wider uppercase
                     focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
            />
          </div>
        {/if}

        {#if mode !== 'forgot'}
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
        {:else}
          <div class="animate-slide-up">
            <label for="newPassword" class="block text-sm font-medium text-slate-300 mb-1.5">
              Nouveau mot de passe
            </label>
            <input
              id="newPassword"
              type="password"
              bind:value={newPassword}
              placeholder="••••••"
              required
              minlength="6"
              class="w-full px-4 py-3 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500
                     focus:outline-none focus:ring-2 focus:ring-primary-500/50 focus:border-primary-500 transition-all"
            />
          </div>
        {/if}

        {#if error}
          <div class="bg-red-500/10 border border-red-500/30 text-red-400 text-sm rounded-xl px-4 py-3 animate-fade-in">
            {error}
          </div>
        {/if}

        {#if success}
          <div class="bg-green-500/10 border border-green-500/30 text-green-400 text-sm rounded-xl px-4 py-3 animate-fade-in">
            {success}
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
          {:else if mode === 'login'}
            Se connecter
          {:else if mode === 'register'}
            S'inscrire
          {:else}
            Réinitialiser le mot de passe
          {/if}
        </button>
      </form>

      <!-- Liens de navigation -->
      <div class="text-center text-sm mt-6 space-y-2">
        {#if mode === 'login'}
          <p class="text-slate-400">
            Pas encore de compte ?
            <button on:click={() => switchMode('register')} class="text-primary-400 hover:text-primary-300 font-medium ml-1">
              S'inscrire
            </button>
          </p>
          <p>
            <button on:click={() => switchMode('forgot')} class="text-slate-500 hover:text-slate-300 transition-colors">
              Mot de passe oublié ?
            </button>
          </p>
        {:else if mode === 'register'}
          <p class="text-slate-400">
            Déjà un compte ?
            <button on:click={() => switchMode('login')} class="text-primary-400 hover:text-primary-300 font-medium ml-1">
              Se connecter
            </button>
          </p>
        {/if}
      </div>
    </div>
  </div>
</div>
{/if}
