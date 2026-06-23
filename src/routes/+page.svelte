<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";

  type RunningGame = { slug: string; name: string };
  type IgnoredGame = { server_id: string; slug: string; name: string };
  type UiServer = { id: string; url: string; org_name: string; status_override: string };
  type UiState = {
    servers: UiServer[];
    global_status: string;
    logged_in: boolean;
    games_count: number;
    running: RunningGame[];
    ignored: IgnoredGame[];
  };
  type LinkInfo = { user_code: string; verification_url: string };
  type ServerStatus = "disconnected" | "connecting" | "connected" | "logged_out";
  type StatusEvent = { server_id: string; status: ServerStatus; detail: string };

  let servers = $state<UiServer[]>([]);
  let statuses = $state<Record<string, { status: ServerStatus; detail: string }>>({});
  let running = $state<RunningGame[]>([]);
  let ignored = $state<IgnoredGame[]>([]);
  let gamesCount = $state(0);
  let globalStatus = $state("online");
  let autostart = $state(false);

  let serverUrl = $state("");
  let error = $state("");
  let linking = $state(false);
  let adding = $state(false);
  let pairing = $state<LinkInfo | null>(null);

  type UpdateInfo = {
    current: string;
    latest: string | null;
    update_available: boolean;
    releases_url: string;
  };

  // Installed version (instant, local) and best-effort update status.
  let appVersion = $state("");
  let update = $state<UpdateInfo | null>(null);

  async function checkForUpdate() {
    try {
      appVersion = await getVersion();
    } catch (e) {
      console.warn("getVersion failed", e);
    }
    try {
      // One cached, short-timeout GET; runs only when this window mounts.
      update = await invoke<UpdateInfo>("check_for_update");
    } catch (e) {
      console.warn("check_for_update failed", e);
    }
  }

  async function refreshAutostart() {
    try {
      autostart = await invoke<boolean>("get_autostart");
    } catch (e) {
      console.warn("get_autostart failed", e);
    }
  }

  async function toggleAutostart() {
    const next = !autostart;
    try {
      await invoke("set_autostart", { enabled: next });
      autostart = next;
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshState() {
    const s = await invoke<UiState>("get_state");
    servers = s.servers;
    globalStatus = s.global_status;
    gamesCount = s.games_count;
    running = s.running;
    ignored = s.ignored;
    if (servers.length > 0) refreshAutostart();
    // Drop status entries for servers that no longer exist.
    const ids = new Set(servers.map((x) => x.id));
    for (const id of Object.keys(statuses)) if (!ids.has(id)) delete statuses[id];
  }

  function statusOf(id: string): { status: ServerStatus; detail: string } {
    return statuses[id] ?? { status: "connecting", detail: "" };
  }

  function statusLabel(id: string): string {
    const { status, detail } = statusOf(id);
    if (status === "connected") return "online";
    if (status === "connecting") return "connecting…";
    if (status === "logged_out") return "not linked";
    return detail || "reconnecting…";
  }

  onMount(() => {
    refreshState();
    checkForUpdate();
    const interval = setInterval(refreshState, 4000);
    const unsubs = [
      listen<StatusEvent>("kfire://status", (e) => {
        const { server_id, status, detail } = e.payload;
        // server_id empty => a pairing attempt that was denied or expired.
        if (!server_id) {
          if (status === "logged_out") {
            error = detail || "linking failed";
            linking = false;
            pairing = null;
          }
          return;
        }
        statuses[server_id] = { status, detail };
        if (status === "connected") {
          pairing = null;
          linking = false;
          adding = false;
          serverUrl = "";
          refreshState();
        }
        if (status === "logged_out") {
          // This server's session ended (unlinked / token dead).
          refreshState();
        }
      }),
      listen("kfire://detection", () => refreshState()),
    ];
    return () => {
      clearInterval(interval);
      unsubs.forEach((u) => u.then((fn) => fn()));
    };
  });

  async function startLink(event: Event) {
    event.preventDefault();
    error = "";
    linking = true;
    try {
      pairing = await invoke<LinkInfo>("start_link", { serverUrl });
    } catch (e) {
      error = String(e);
      linking = false;
    }
  }

  function cancel() {
    pairing = null;
    linking = false;
    adding = false;
    serverUrl = "";
  }

  async function unlink(id: string) {
    await invoke("unlink_server", { serverId: id });
    delete statuses[id];
    await refreshState();
  }

  async function setGlobal(status: string) {
    globalStatus = status;
    try {
      await invoke("set_global_status", { status });
    } catch (e) {
      error = String(e);
    }
    await refreshState();
  }

  async function setServer(id: string, status: string) {
    try {
      await invoke("set_server_status", { serverId: id, status });
    } catch (e) {
      error = String(e);
    }
    await refreshState();
  }
</script>

<main>
  <header>
    <h1>KFIRE</h1>
    {#if servers.length > 0}
      <span class="muted small">{servers.length} server{servers.length > 1 ? "s" : ""}</span>
    {/if}
  </header>

  {#if pairing}
    <section class="pairing">
      <p>We opened your browser to confirm the link.</p>
      <p class="muted">If it didn't open, go to:</p>
      <a class="link" href={pairing.verification_url} target="_blank">{pairing.verification_url}</a>
      <p class="muted">and approve the code:</p>
      <p class="code">{pairing.user_code}</p>
      <p class="muted small">Waiting for approval…</p>
      <button class="secondary" onclick={cancel}>Cancel</button>
    </section>
  {:else if servers.length === 0}
    <form onsubmit={startLink}>
      <p class="muted">Connect this app to your organization's KFIRE server.</p>
      <label>
        Server address
        <input type="url" placeholder="https://kfire.example.org" bind:value={serverUrl} required />
      </label>
      {#if error}<p class="error">{error}</p>{/if}
      <button type="submit" disabled={linking}>{linking ? "Opening browser…" : "Link this device"}</button>
    </form>
  {:else}
    <section class="session">
      <div class="global-status">
        <span>Status</span>
        <select value={globalStatus} onchange={(e) => setGlobal(e.currentTarget.value)}>
          <option value="online">Online</option>
          <option value="invisible">Invisible</option>
          <option value="offline">Offline</option>
        </select>
      </div>

      <h2>Servers</h2>
      <ul class="servers">
        {#each servers as srv (srv.id)}
          <li class="server">
            <span class="dot {statusOf(srv.id).status}"></span>
            <div class="server-info">
              <span class="server-name">{srv.org_name || srv.url}</span>
              <span class="muted small">{statusLabel(srv.id)}</span>
            </div>
            <select
              class="srv-status"
              value={srv.status_override}
              onchange={(e) => setServer(srv.id, e.currentTarget.value)}
              title="Status for this server"
            >
              <option value="inherit">Use global</option>
              <option value="online">Online</option>
              <option value="invisible">Invisible</option>
              <option value="offline">Offline</option>
            </select>
            <button class="link-btn" onclick={() => unlink(srv.id)} title="Unlink this server">Unlink</button>
          </li>
        {/each}
      </ul>

      <h2>Now playing</h2>
      {#if running.length === 0}
        <p class="muted">No game detected.</p>
      {:else}
        <ul>{#each running as game (game.slug)}
          <li class="np">
            <span class="np-name">🎮 {game.name}</span>
            <span class="np-actions">
              <button class="mini" onclick={() => invoke('stop_game', { slug: game.slug }).then(refreshState)}>Stop</button>
              <button class="mini" onclick={() => invoke('ignore_game', { slug: game.slug, ignored: true }).then(refreshState)} title="Toujours ignorer ce jeu">Ignorer</button>
            </span>
          </li>
        {/each}</ul>
      {/if}

      {#if ignored.length > 0}
        <h2>Jeux ignorés</h2>
        <ul>{#each ignored as g (g.server_id + g.slug)}
          <li class="np">
            <span class="np-name">{g.name}</span>
            <button class="mini" onclick={() => invoke('ignore_game', { slug: g.slug, ignored: false }).then(refreshState)}>Réactiver</button>
          </li>
        {/each}</ul>
      {/if}

      <p class="muted small">{gamesCount.toLocaleString()} games across your catalogs</p>

      {#if adding}
        <form onsubmit={startLink}>
          <label>
            Server address
            <input type="url" placeholder="https://kfire.example.org" bind:value={serverUrl} required />
          </label>
          {#if error}<p class="error">{error}</p>{/if}
          <div class="row">
            <button type="submit" disabled={linking}>{linking ? "Opening browser…" : "Link"}</button>
            <button type="button" class="secondary" onclick={cancel}>Cancel</button>
          </div>
        </form>
      {:else}
        <button class="secondary" onclick={() => { adding = true; error = ""; }}>Add a server</button>
      {/if}

      <label class="toggle">
        <input type="checkbox" checked={autostart} onchange={toggleAutostart} />
        <span>Launch KFIRE at startup</span>
      </label>
    </section>
  {/if}

  <footer>
    <p class="version">
      <span>KFIRE{appVersion ? ` v${appVersion}` : ""}</span>
      {#if update?.update_available && update.latest}
        <button
          type="button"
          class="version-link"
          onclick={() => update && openUrl(update.releases_url)}
        >
          {update.latest} available
        </button>
      {:else if update?.latest}
        <span class="up-to-date">up to date</span>
      {/if}
    </p>
    <p>Runs in the tray - closing this window keeps KFIRE running.</p>
  </footer>
</main>

<style>
  :root {
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
    color: #e5e7eb;
    background-color: #0b0e14;
    -webkit-font-smoothing: antialiased;
  }
  main { display: flex; flex-direction: column; gap: 1.4rem; padding: 1.5rem; min-height: 100vh; box-sizing: border-box; }
  header { display: flex; align-items: baseline; justify-content: space-between; }
  h1 { margin: 0; font-size: 1.4rem; letter-spacing: 0.12em; color: #f97316; }
  h2 { margin: 1rem 0 0.4rem; font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.08em; color: #9ca3af; }
  form, .session, .pairing { display: flex; flex-direction: column; gap: 0.9rem; }
  .row { display: flex; gap: 0.6rem; }
  .row button { flex: 1; }
  label { display: flex; flex-direction: column; gap: 0.3rem; font-size: 0.8rem; color: #9ca3af; }
  input { padding: 0.55em 0.8em; font-size: 0.95rem; color: #e5e7eb; background: #151a23; border: 1px solid #2a3140; border-radius: 8px; outline: none; }
  input:focus { border-color: #f97316; }
  button { margin-top: 0.4rem; padding: 0.6em 1.2em; font-size: 0.95rem; font-weight: 600; color: #0b0e14; background: #f97316; border: none; border-radius: 8px; cursor: pointer; }
  button:hover { background: #fb923c; }
  button:disabled { opacity: 0.6; cursor: wait; }
  button.secondary { background: transparent; color: #9ca3af; border: 1px solid #2a3140; }
  button.secondary:hover { color: #e5e7eb; border-color: #4b5563; }
  ul { margin: 0; padding: 0; list-style: none; }
  li { padding: 0.5em 0.8em; background: #151a23; border: 1px solid #2a3140; border-radius: 8px; margin-bottom: 0.4rem; }
  ul.servers .server { display: flex; align-items: center; gap: 0.6rem; }
  .server-info { display: flex; flex-direction: column; gap: 0.1rem; flex: 1; min-width: 0; }
  .server-name { font-weight: 600; color: #e5e7eb; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dot { width: 0.6rem; height: 0.6rem; border-radius: 50%; flex-shrink: 0; background: #6b7280; }
  .dot.connected { background: #22c55e; }
  .dot.connecting { background: #eab308; }
  .dot.disconnected { background: #f97316; }
  .dot.logged_out { background: #6b7280; }
  .link-btn { margin: 0; padding: 0.3em 0.7em; font-size: 0.75rem; font-weight: 600; background: transparent; color: #9ca3af; border: 1px solid #2a3140; border-radius: 6px; }
  .link-btn:hover { color: #ef4444; border-color: #ef4444; background: transparent; }
  .global-status { display: flex; align-items: center; justify-content: space-between; gap: 0.6rem; padding: 0.5em 0.8em; background: #151a23; border: 1px solid #2a3140; border-radius: 8px; font-size: 0.85rem; color: #9ca3af; }
  select { padding: 0.35em 0.5em; font-size: 0.8rem; color: #e5e7eb; background: #0b0e14; border: 1px solid #2a3140; border-radius: 6px; outline: none; cursor: pointer; }
  select:focus { border-color: #f97316; }
  .srv-status { flex-shrink: 0; }
  .np { display: flex; align-items: center; justify-content: space-between; gap: 0.6rem; }
  .np-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .np-actions { display: flex; gap: 0.4rem; flex-shrink: 0; }
  .mini { margin: 0; padding: 0.3em 0.7em; font-size: 0.75rem; font-weight: 600; background: transparent; color: #9ca3af; border: 1px solid #2a3140; border-radius: 6px; }
  .mini:hover { color: #e5e7eb; border-color: #4b5563; background: transparent; }
  .code { font-size: 1.6rem; font-weight: 700; letter-spacing: 0.18em; color: #f97316; text-align: center; margin: 0.2rem 0; }
  .link { color: #fb923c; font-size: 0.85rem; word-break: break-all; }
  .muted { color: #6b7280; font-size: 0.85rem; margin: 0; }
  .muted.small { font-size: 0.75rem; }
  .error { color: #ef4444; font-size: 0.85rem; margin: 0; }
  .toggle { flex-direction: row; align-items: center; gap: 0.5rem; cursor: pointer; color: #9ca3af; font-size: 0.85rem; margin-top: 0.4rem; }
  .toggle input { accent-color: #f97316; width: 1rem; height: 1rem; cursor: pointer; }
  footer { margin-top: auto; display: flex; flex-direction: column; gap: 0.3rem; }
  footer p { margin: 0; font-size: 0.75rem; color: #4b5563; text-align: center; }
  .version { display: flex; align-items: center; justify-content: center; gap: 0.5rem; color: #6b7280; }
  .version > span:first-child { letter-spacing: 0.04em; }
  .version-link { margin: 0; padding: 0; font-size: 0.75rem; font-weight: 600; color: #f97316; background: transparent; border: none; cursor: pointer; }
  .version-link:hover { color: #fb923c; background: transparent; text-decoration: underline; }
  .up-to-date { color: #22c55e; }
</style>
