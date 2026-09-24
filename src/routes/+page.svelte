<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, TauriEvent } from "@tauri-apps/api/event";
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
    expired_server_url: string | null;
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
  // Address of a link the server refused: offered back once, not forced on
  // every 4 s refresh (the member may want to type another one).
  let expiredUrl = $state<string | null>(null);
  let expiredPrefilled = false;
  let error = $state("");
  let linking = $state(false);
  let adding = $state(false);
  let pairing = $state<LinkInfo | null>(null);

  type HsStatus = {
    supported: boolean;
    enabled: boolean;
    config_path: string;
    config_block: string;
    install_dir: string | null;
  };

  let hs = $state<HsStatus | null>(null);
  let hsBusy = $state(false);
  let hsError = $state("");

  async function loadHs() {
    try {
      hs = await invoke<HsStatus>("hs_status");
    } catch {
      hs = null;
    }
  }

  async function toggleHs(next: boolean) {
    hsBusy = true;
    hsError = "";
    try {
      await invoke("hs_set_enabled", { enabled: next });
      await loadHs();
    } catch (e) {
      hsError = String(e);
    } finally {
      hsBusy = false;
    }
  }

  type LolStatus = { enabled: boolean; watching: boolean };

  let lol = $state<LolStatus | null>(null);
  let lolBusy = $state(false);
  let lolError = $state("");

  async function loadLol() {
    try {
      lol = await invoke<LolStatus>("lol_status");
    } catch {
      lol = null;
    }
  }

  async function toggleLol(next: boolean) {
    lolBusy = true;
    lolError = "";
    try {
      await invoke("lol_set_enabled", { enabled: next });
      await loadLol();
    } catch (e) {
      lolError = String(e);
    } finally {
      lolBusy = false;
    }
  }

  type RlStatus = {
    supported: boolean;
    enabled: boolean;
    config_path: string;
    config_block: string;
    install_dir: string | null;
    player_name: string;
    last_mismatch: string;
    watching: boolean;
    socket_connected: boolean;
    decoded: number;
  };

  type RlLive = {
    last_mismatch: string;
    watching: boolean;
    socket_connected: boolean;
    decoded: number;
  };

  let rl = $state<RlStatus | null>(null);
  let rlBusy = $state(false);
  let rlError = $state("");
  let rlName = $state("");

  async function loadRl() {
    try {
      const s = await invoke<Omit<RlStatus, "watching" | "socket_connected" | "decoded">>(
        "rl_status",
      );
      const live = await invoke<RlLive>("rl_live");
      rl = {
        ...s,
        watching: live.watching,
        socket_connected: live.socket_connected,
        decoded: live.decoded,
      };
      rlName = rl.player_name;
    } catch {
      rl = null;
    }
  }

  async function saveRlName() {
    try {
      await invoke("rl_set_player_name", { name: rlName });
      await loadRl();
    } catch (e) {
      rlError = String(e);
    }
  }

  async function toggleRl(next: boolean) {
    rlBusy = true;
    rlError = "";
    try {
      await invoke("rl_set_enabled", { enabled: next });
      await loadRl();
    } catch (e) {
      rlError = String(e);
    } finally {
      rlBusy = false;
    }
  }

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
    expiredUrl = s.expired_server_url;
    if (expiredUrl && servers.length === 0 && !serverUrl && !expiredPrefilled) {
      serverUrl = expiredUrl;
      expiredPrefilled = true;
    }
    if (servers.length > 0) refreshAutostart();
    // Drop status entries for servers that no longer exist.
    const ids = new Set(servers.map((x) => x.id));
    for (const id of Object.keys(statuses)) if (!ids.has(id)) delete statuses[id];

    await pollRlMismatch();
  }

  // Les champs Rocket League qui changent sans que le membre ait rien fait :
  // on les sonde à part plutôt que de rappeler rl_status, qui peut énumérer
  // les processus de la machine pour trouver l'installation. Rappelé par
  // l'intervalle existant ET quand la fenêtre reprend le focus, puisque
  // c'est exactement l'instant où le membre regarde l'écran.
  async function pollRlMismatch() {
    if (rl) {
      try {
        const live = await invoke<RlLive>("rl_live");
        rl.last_mismatch = live.last_mismatch;
        rl.watching = live.watching;
        rl.socket_connected = live.socket_connected;
        rl.decoded = live.decoded;
      } catch {
        // Le reste de l'écran RL reste inchangé si l'appel échoue.
      }
    }
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
    loadHs();
    loadRl();
    loadLol();
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
      // Le moment où il regarde vraiment l'écran : plus fiable qu'un minuteur
      // qu'une fenêtre mise en arrière-plan pourrait voir ralentir.
      listen(TauriEvent.WINDOW_FOCUS, () => pollRlMismatch()),
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
      {#if expiredUrl}
        <p class="error">
          Your session on {expiredUrl} expired. Link this device again to resume tracking.
        </p>
      {:else}
        <p class="muted">Connect this app to your organization's KFIRE server.</p>
      {/if}
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

      {#if hs?.supported}
        <h2>Suivi des parties Hearthstone</h2>

        {#if hs.enabled}
          <p class="muted">
            Le suivi est actif. KFIRE lit le journal du jeu et n'envoie que le mode, le
            résultat, le nombre de tours, votre position et votre héros. Le pseudo de votre
            adversaire et les cartes jouées ne quittent jamais cet ordinateur.
          </p>
          <button class="secondary" disabled={hsBusy} onclick={() => toggleHs(false)}>
            Désactiver et retirer le fichier
          </button>
        {:else}
          <p class="muted">
            Hearthstone n'écrit ses journaux détaillés que si ce fichier existe. KFIRE va
            l'écrire ici :
          </p>
          <pre class="hs-pre">{hs.config_path}</pre>
          <p class="muted">avec exactement ce contenu :</p>
          <pre class="hs-pre">{hs.config_block}</pre>
          <p class="muted">
            Seuls le mode, le résultat, le nombre de tours, votre position et votre héros
            sont envoyés. Le pseudo de votre adversaire et les cartes jouées ne quittent
            jamais cet ordinateur.
          </p>
          <button disabled={hsBusy} onclick={() => toggleHs(true)}>Activer le suivi</button>
        {/if}

        {#if hsError}
          <p class="error" role="alert">{hsError}</p>
        {/if}

        {#if !hs.install_dir}
          <p class="muted small">
            Dossier d'installation introuvable. Lancez Hearthstone une fois, puis rouvrez cet
            écran.
          </p>
        {/if}
      {/if}

      {#if rl}
        <h2>Suivi des parties Rocket League</h2>

        <label>
          <span>Votre pseudo Rocket League (celui affiché en jeu)</span>
          <input
            type="text"
            bind:value={rlName}
            onblur={saveRlName}
            placeholder="Le pseudo exact affiché en jeu"
          />
        </label>
        <p class="muted small">
          Ce pseudo ne quitte jamais cet ordinateur : il sert uniquement à retrouver votre
          ligne dans la feuille de match, jamais envoyé au serveur.
        </p>

        {#if rl.enabled}
          {#if !rl.watching}
            <p class="muted small">Rocket League n'est pas lancé.</p>
          {:else if !rl.socket_connected}
            <p class="warning" role="status">
              En attente de la socket du jeu. Si tu viens d'activer le suivi, redémarre Rocket
              League : le jeu ne lit sa configuration qu'au démarrage.
            </p>
          {:else if rl.decoded > 0}
            <p class="muted small">Connecté à Rocket League, {rl.decoded} messages reçus.</p>
          {:else}
            <p class="warning" role="status">
              Connecté à Rocket League, mais le jeu n'envoie rien. Lance une partie : la
              socket ne parle qu'en match.
            </p>
          {/if}
        {/if}

        {#if rl.last_mismatch}
          <p class="warning" role="status">
            Votre dernier match n'a correspondu à personne. Le jeu a vu ces pseudos :
            {rl.last_mismatch}. L'un d'eux est le vôtre : copiez-le exactement dans le champ
            ci-dessus.
          </p>
        {/if}

        {#if !rl.install_dir}
          <p class="muted small">
            Dossier d'installation introuvable. Lancez Rocket League une fois, puis rouvrez cet
            écran.
          </p>
        {:else if rl.enabled}
          <p class="muted">
            Le suivi est actif. KFIRE lit la socket de statistiques du jeu et n'envoie que le
            résumé du match : mode, score, buts, passes, arrêts, tirs, démos et durée. Les
            pseudos des autres joueurs, coéquipiers comme adversaires, ne quittent jamais cet
            ordinateur.
          </p>
          <button class="secondary" disabled={rlBusy} onclick={() => toggleRl(false)}>
            Désactiver et retirer le fichier
          </button>
        {:else}
          <p class="muted">
            Rocket League n'ouvre sa socket de statistiques que si ce fichier existe. KFIRE va
            l'écrire ici :
          </p>
          <pre class="hs-pre">{rl.config_path}</pre>
          <p class="muted">avec exactement ce contenu :</p>
          <pre class="hs-pre">{rl.config_block}</pre>
          <p class="muted">
            Seul le résumé du match sera envoyé : mode, score, buts, passes, arrêts, tirs,
            démos et durée. Les pseudos des autres joueurs ne quittent jamais cet ordinateur.
          </p>
          <button disabled={rlBusy || !rlName.trim()} onclick={() => toggleRl(true)}>
            Activer le suivi
          </button>
        {/if}

        <p class="muted small">
          Rocket League ne lit ce fichier qu'à son démarrage : activer le suivi pendant une
          partie ne prendra effet qu'au prochain lancement du jeu.
        </p>

        {#if rlError}
          <p class="error" role="alert">{rlError}</p>
        {/if}
      {/if}

      <h2>Suivi des parties League of Legends</h2>

      {#if lol?.enabled}
        <p class="muted">
          Le suivi est actif. Pendant une partie, KFIRE lit l'API que League of Legends
          ouvre sur cet ordinateur et n'envoie que votre champion, votre niveau, votre
          KDA, vos sbires, votre or et le temps de jeu. Les pseudos des neuf autres
          joueurs ne quittent jamais cette machine.
        </p>
        {#if lol.watching}
          <p class="muted small">Partie en cours détectée.</p>
        {:else}
          <p class="muted small">
            Aucune partie en cours. L'API du jeu n'existe que pendant une partie : c'est
            normal entre deux parties.
          </p>
        {/if}
        <button class="secondary" disabled={lolBusy} onclick={() => toggleLol(false)}>
          Désactiver le suivi
        </button>
      {:else}
        <p class="muted">
          Affiche votre partie en cours sur la page « Jeux en live » du portail, avec votre
          champion, votre KDA, vos sbires et votre or. Rien à configurer : le jeu dit
          lui-même quel joueur vous êtes, donc aucun pseudo à saisir.
        </p>
        <button class="secondary" disabled={lolBusy} onclick={() => toggleLol(true)}>
          Activer le suivi
        </button>
      {/if}

      {#if lolError}
        <p class="error" role="alert">{lolError}</p>
      {/if}
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
  .warning { color: #f59e0b; font-size: 0.85rem; margin: 0; }
  .toggle { flex-direction: row; align-items: center; gap: 0.5rem; cursor: pointer; color: #9ca3af; font-size: 0.85rem; margin-top: 0.4rem; }
  .toggle input { accent-color: #f97316; width: 1rem; height: 1rem; cursor: pointer; }
  footer { margin-top: auto; display: flex; flex-direction: column; gap: 0.3rem; }
  footer p { margin: 0; font-size: 0.75rem; color: #4b5563; text-align: center; }
  .version { display: flex; align-items: center; justify-content: center; gap: 0.5rem; color: #6b7280; }
  .version > span:first-child { letter-spacing: 0.04em; }
  .version-link { margin: 0; padding: 0; font-size: 0.75rem; font-weight: 600; color: #f97316; background: transparent; border: none; cursor: pointer; }
  .version-link:hover { color: #fb923c; background: transparent; text-decoration: underline; }
  .up-to-date { color: #22c55e; }
  .hs-pre { margin: 0; padding: 0.6em 0.8em; background: #151a23; border: 1px solid #2a3140; border-radius: 8px; color: #e5e7eb; font-size: 0.8rem; overflow-x: auto; white-space: pre; }
</style>
