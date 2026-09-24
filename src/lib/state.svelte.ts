import { invoke } from "@tauri-apps/api/core";
import { listen, TauriEvent } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { t } from "./i18n";
import type {
  HsStatus, IgnoredGame, LinkInfo, LolStatus, RlLive, RlStatus, RunningGame,
  ServerStatus, StatusEvent, Tab, UiServer, UiState, UpdateInfo,
} from "./types";

const TABS: Tab[] = ["home", "hearthstone", "rocket-league", "league-of-legends", "settings"];
const TAB_KEY = "kfire.tab";

function savedTab(): Tab {
  try {
    const v = localStorage.getItem(TAB_KEY) as Tab | null;
    return v && TABS.includes(v) ? v : "home";
  } catch {
    return "home";
  }
}

class AppState {
  loaded = $state(false);
  tab = $state<Tab>(savedTab());
  now = $state(new Date());

  servers = $state<UiServer[]>([]);
  statuses = $state<Record<string, { status: ServerStatus; detail: string }>>({});
  running = $state<RunningGame[]>([]);
  ignored = $state<IgnoredGame[]>([]);
  gamesCount = $state(0);
  globalStatus = $state("online");
  autostart = $state(false);

  serverUrl = $state("");
  // Address of a link the server refused: offered back once, not forced on
  // every 4 s refresh (the member may want to type another one).
  expiredUrl = $state<string | null>(null);
  private expiredPrefilled = false;
  error = $state("");
  linking = $state(false);
  adding = $state(false);
  pairing = $state<LinkInfo | null>(null);

  appVersion = $state("");
  update = $state<UpdateInfo | null>(null);

  hs = $state<HsStatus | null>(null);
  hsBusy = $state(false);
  hsError = $state("");
  rl = $state<RlStatus | null>(null);
  rlBusy = $state(false);
  rlError = $state("");
  rlName = $state("");
  lol = $state<LolStatus | null>(null);
  lolBusy = $state(false);
  lolError = $state("");

  setTab(tab: Tab) {
    this.tab = tab;
    try {
      localStorage.setItem(TAB_KEY, tab);
    } catch {
      // Storage refused: the tab is simply not remembered.
    }
  }

  statusOf(id: string): { status: ServerStatus; detail: string } {
    return this.statuses[id] ?? { status: "connecting", detail: "" };
  }

  statusLabel(id: string): string {
    const { status, detail } = this.statusOf(id);
    if (status === "connected") return t("conn.connected");
    if (status === "connecting") return t("conn.connecting");
    if (status === "logged_out") return t("conn.loggedOut");
    return detail || t("conn.reconnecting");
  }

  async refresh() {
    this.now = new Date();
    const s = await invoke<UiState>("get_state");
    this.servers = s.servers;
    this.globalStatus = s.global_status;
    this.gamesCount = s.games_count;
    this.running = s.running;
    this.ignored = s.ignored;
    this.expiredUrl = s.expired_server_url;
    if (this.expiredUrl && this.servers.length === 0 && !this.serverUrl && !this.expiredPrefilled) {
      this.serverUrl = this.expiredUrl;
      this.expiredPrefilled = true;
    }
    if (this.servers.length > 0) this.refreshAutostart();
    // Drop status entries for servers that no longer exist.
    const ids = new Set(this.servers.map((x) => x.id));
    for (const id of Object.keys(this.statuses)) if (!ids.has(id)) delete this.statuses[id];
    this.loaded = true;
    await this.pollRl();
  }

  async refreshAutostart() {
    try {
      this.autostart = await invoke<boolean>("get_autostart");
    } catch (e) {
      console.warn("get_autostart failed", e);
    }
  }

  async toggleAutostart() {
    const next = !this.autostart;
    try {
      await invoke("set_autostart", { enabled: next });
      this.autostart = next;
    } catch (e) {
      this.error = String(e);
    }
  }

  async checkForUpdate() {
    try {
      this.appVersion = await getVersion();
    } catch (e) {
      console.warn("getVersion failed", e);
    }
    try {
      // One cached, short-timeout GET; runs only when this window mounts.
      this.update = await invoke<UpdateInfo>("check_for_update");
    } catch (e) {
      console.warn("check_for_update failed", e);
    }
  }

  async loadHs() {
    try {
      this.hs = await invoke<HsStatus>("hs_status");
    } catch {
      this.hs = null;
    }
  }

  async toggleHs(next: boolean) {
    this.hsBusy = true;
    this.hsError = "";
    try {
      await invoke("hs_set_enabled", { enabled: next });
      await this.loadHs();
    } catch (e) {
      this.hsError = String(e);
    } finally {
      this.hsBusy = false;
    }
  }

  async loadRl() {
    const previous = this.rl?.player_name;
    try {
      const s = await invoke<Omit<RlStatus, keyof RlLive | "last_mismatch"> & { last_mismatch: string }>("rl_status");
      const live = await invoke<RlLive>("rl_live");
      this.rl = { ...s, ...live };
      // Ne pas écraser un nom que le membre est en train de taper.
      if (previous === undefined || this.rlName === previous) this.rlName = this.rl.player_name;
    } catch {
      this.rl = null;
    }
  }

  // Les champs Rocket League qui changent sans que le membre ait rien fait :
  // sondés à part plutôt que de rappeler rl_status, qui peut énumérer les
  // processus de la machine pour trouver l'installation.
  async pollRl() {
    if (!this.rl) return;
    try {
      const live = await invoke<RlLive>("rl_live");
      Object.assign(this.rl, live);
    } catch {
      // Le reste de l'écran RL reste inchangé si l'appel échoue.
    }
  }

  async saveRlName() {
    try {
      await invoke("rl_set_player_name", { name: this.rlName });
      await this.loadRl();
    } catch (e) {
      this.rlError = String(e);
    }
  }

  async toggleRl(next: boolean) {
    this.rlBusy = true;
    this.rlError = "";
    try {
      await invoke("rl_set_enabled", { enabled: next });
      await this.loadRl();
    } catch (e) {
      this.rlError = String(e);
    } finally {
      this.rlBusy = false;
    }
  }

  async loadLol() {
    try {
      this.lol = await invoke<LolStatus>("lol_status");
    } catch {
      this.lol = null;
    }
  }

  async toggleLol(next: boolean) {
    this.lolBusy = true;
    this.lolError = "";
    try {
      await invoke("lol_set_enabled", { enabled: next });
      await this.loadLol();
    } catch (e) {
      this.lolError = String(e);
    } finally {
      this.lolBusy = false;
    }
  }

  async startLink(event: Event) {
    event.preventDefault();
    this.error = "";
    this.linking = true;
    try {
      this.pairing = await invoke<LinkInfo>("start_link", { serverUrl: this.serverUrl });
    } catch (e) {
      this.error = String(e);
      this.linking = false;
    }
  }

  cancel() {
    this.pairing = null;
    this.linking = false;
    this.adding = false;
    this.serverUrl = "";
  }

  async unlink(id: string) {
    await invoke("unlink_server", { serverId: id });
    delete this.statuses[id];
    await this.refresh();
  }

  async setGlobal(status: string) {
    this.globalStatus = status;
    try {
      await invoke("set_global_status", { status });
    } catch (e) {
      this.error = String(e);
    }
    await this.refresh();
  }

  async setServer(id: string, status: string) {
    try {
      await invoke("set_server_status", { serverId: id, status });
    } catch (e) {
      this.error = String(e);
    }
    await this.refresh();
  }

  async stopGame(slug: string) {
    await invoke("stop_game", { slug });
    await this.refresh();
  }

  async ignoreGame(slug: string, ignored: boolean) {
    await invoke("ignore_game", { slug, ignored });
    await this.refresh();
  }

  /** Starts polling and listening; returns the cleanup for onMount. */
  init(): () => void {
    this.refresh().catch((e) => console.warn("refresh failed", e));
    this.checkForUpdate();
    this.loadHs();
    this.loadRl();
    this.loadLol();
    const interval = setInterval(() => {
      this.refresh().catch((e) => console.warn("refresh failed", e));
    }, 4000);
    const unsubs = [
      listen<StatusEvent>("kfire://status", (e) => {
        const { server_id, status, detail } = e.payload;
        // server_id empty => a pairing attempt that was denied or expired.
        if (!server_id) {
          if (status === "logged_out") {
            this.error = detail || t("link.failed");
            this.linking = false;
            this.pairing = null;
          }
          return;
        }
        this.statuses[server_id] = { status, detail };
        if (status === "connected") {
          this.pairing = null;
          this.linking = false;
          this.adding = false;
          this.serverUrl = "";
          this.refresh().catch((e) => console.warn("refresh failed", e));
        }
        if (status === "logged_out") this.refresh().catch((e) => console.warn("refresh failed", e));
      }),
      listen("kfire://detection", () => {
        this.refresh().catch((e) => console.warn("refresh failed", e));
        this.loadHs();
        this.loadRl();
      }),
      // The moment the member actually looks at the window.
      listen(TauriEvent.WINDOW_FOCUS, () => {
        this.loadHs();
        this.loadRl();
      }),
    ];
    return () => {
      clearInterval(interval);
      unsubs.forEach((u) => u.then((fn) => fn()));
    };
  }
}

export const app = new AppState();
