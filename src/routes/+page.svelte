<script lang="ts">
  // Minimal stub UI. TODO(mvp):
  //  - real login against the server REST API (kfire-protocol/openapi.yaml)
  //  - WebSocket connection status from the Rust side (tauri events)
  //  - OAuth account linking (Steam, Battle.net, ...)
  let serverUrl = $state("");
  let username = $state("");
  let password = $state("");
  let status = $state<"disconnected" | "connecting" | "connected">("disconnected");

  async function login(event: Event) {
    event.preventDefault();
    status = "connecting";
    // TODO(mvp): invoke a Rust command that performs POST /api/v1/auth/login
    // and opens the presence WebSocket.
    setTimeout(() => (status = "disconnected"), 800);
  }
</script>

<main>
  <header>
    <h1>KFIRE</h1>
    <span class="status {status}">
      {#if status === "connected"}● connected
      {:else if status === "connecting"}● connecting…
      {:else}● disconnected{/if}
    </span>
  </header>

  <form onsubmit={login}>
    <label>
      Server
      <input
        type="url"
        placeholder="https://kfire.example.org"
        bind:value={serverUrl}
        required
      />
    </label>
    <label>
      Username
      <input type="text" bind:value={username} autocomplete="username" required />
    </label>
    <label>
      Password
      <input
        type="password"
        bind:value={password}
        autocomplete="current-password"
        required
      />
    </label>
    <button type="submit" disabled={status === "connecting"}>Sign in</button>
  </form>

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
    gap: 1.5rem;
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

  .status {
    font-size: 0.8rem;
  }
  .status.disconnected {
    color: #6b7280;
  }
  .status.connecting {
    color: #eab308;
  }
  .status.connected {
    color: #22c55e;
  }

  form {
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
  input:focus {
    border-color: #f97316;
  }

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
  button:hover {
    background: #fb923c;
  }
  button:disabled {
    opacity: 0.6;
    cursor: wait;
  }

  footer p {
    margin: 0;
    font-size: 0.75rem;
    color: #4b5563;
    text-align: center;
  }
</style>
