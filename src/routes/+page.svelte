<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type RunningGame = { slug: string; name: string };
  type UiState = {
    server_url: string | null;
    username: string | null;
    logged_in: boolean;
    games_count: number;
    running: RunningGame[];
  };

  let serverUrl = $state("");
  let username = $state("");
  let password = $state("");
  let error = $state("");
  let busy = $state(false);

  let loggedIn = $state(false);
  let currentUser = $state<string | null>(null);
  let gamesCount = $state(0);
  let running = $state<RunningGame[]>([]);
  let status = $state<"disconnected" | "connecting" | "connected" | "logged_out">("disconnected");
  let statusDetail = $state("");

  async function refreshState() {
    const s = await invoke<UiState>("get_state");
    loggedIn = s.logged_in;
    currentUser = s.username;
    gamesCount = s.games_count;
    running = s.running;
    if (s.server_url && !serverUrl) serverUrl = s.server_url;
    if (s.username && !username) username = s.username;
  }

  onMount(() => {
    refreshState();
    const interval = setInterval(refreshState, 5000);

    const unsubs: Promise<() => void>[] = [
      listen<{ status: typeof status; detail: string }>("kfire://status", (e) => {
        status = e.payload.status;
        statusDetail = e.payload.detail;
        if (status === "logged_out") loggedIn = false;
      }),
      listen("kfire://detection", () => refreshState()),
    ];

    return () => {
      clearInterval(interval);
      unsubs.forEach((u) => u.then((fn) => fn()));
    };
  });

  async function login(event: Event) {
    event.preventDefault();
    error = "";
    busy = true;
    try {
      await invoke("login", { serverUrl, username, password });
      password = "";
      await refreshState();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function logout() {
    await invoke("logout");
    status = "disconnected";
    await refreshState();
  }
</script>

<main>
  <header>
    <h1>KFIRE</h1>
    <span class="status {status}">
      {#if status === "connected"}● connected
      {:else if status === "connecting"}● connecting…
      {:else if status === "logged_out"}● signed out
      {:else}● disconnected{/if}
    </span>
  </header>

  {#if !loggedIn}
    <form onsubmit={login}>
      <label>
        Server
        <input type="url" placeholder="https://kfire.example.org" bind:value={serverUrl} required />
      </label>
      <label>
        Username
        <input type="text" bind:value={username} autocomplete="username" required />
      </label>
      <label>
        Password
        <input type="password" bind:value={password} autocomplete="current-password" required />
      </label>
      {#if error}<p class="error">{error}</p>{/if}
      <button type="submit" disabled={busy}>{busy ? "Signing in…" : "Sign in"}</button>
    </form>
  {:else}
    <section class="session">
      <p class="who">
        <strong>{currentUser}</strong>
        <span class="muted">· {gamesCount.toLocaleString()} games in catalog</span>
      </p>
      {#if statusDetail}<p class="muted">{statusDetail}</p>{/if}

      <h2>Now playing</h2>
      {#if running.length === 0}
        <p class="muted">No game detected.</p>
      {:else}
        <ul>
          {#each running as game (game.slug)}
            <li>🎮 {game.name}</li>
          {/each}
        </ul>
      {/if}

      <button class="secondary" onclick={logout}>Sign out</button>
    </section>
  {/if}

  <footer>
    <p>Runs in the tray — closing this window keeps KFIRE running.</p>
  </footer>
</main>

<style>
  :root {
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
    color: #e5e7eb;
    background-color: #0b0e14;
    -webkit-font-smoothing: antialiased;
  }

  main {
    display: flex;
    flex-direction: column;
    gap: 1.4rem;
    padding: 1.5rem;
    min-height: 100vh;
    box-sizing: border-box;
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }

  h1 {
    margin: 0;
    font-size: 1.4rem;
    letter-spacing: 0.12em;
    color: #f97316;
  }

  h2 {
    margin: 1rem 0 0.4rem;
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #9ca3af;
  }

  .status { font-size: 0.8rem; }
  .status.disconnected, .status.logged_out { color: #6b7280; }
  .status.connecting { color: #eab308; }
  .status.connected { color: #22c55e; }

  form, .session {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.8rem;
    color: #9ca3af;
  }

  input {
    padding: 0.55em 0.8em;
    font-size: 0.95rem;
    color: #e5e7eb;
    background: #151a23;
    border: 1px solid #2a3140;
    border-radius: 8px;
    outline: none;
  }
  input:focus { border-color: #f97316; }

  button {
    margin-top: 0.4rem;
    padding: 0.6em 1.2em;
    font-size: 0.95rem;
    font-weight: 600;
    color: #0b0e14;
    background: #f97316;
    border: none;
    border-radius: 8px;
    cursor: pointer;
  }
  button:hover { background: #fb923c; }
  button:disabled { opacity: 0.6; cursor: wait; }
  button.secondary {
    background: transparent;
    color: #9ca3af;
    border: 1px solid #2a3140;
  }
  button.secondary:hover { color: #e5e7eb; border-color: #4b5563; }

  ul { margin: 0; padding: 0; list-style: none; }
  li {
    padding: 0.5em 0.8em;
    background: #151a23;
    border: 1px solid #2a3140;
    border-radius: 8px;
    margin-bottom: 0.4rem;
  }

  .who { margin: 0; }
  .muted { color: #6b7280; font-size: 0.85rem; margin: 0; }
  .error { color: #ef4444; font-size: 0.85rem; margin: 0; }

  footer { margin-top: auto; }
  footer p {
    margin: 0;
    font-size: 0.75rem;
    color: #4b5563;
    text-align: center;
  }
</style>
