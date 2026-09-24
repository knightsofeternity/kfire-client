import { mockIPC } from "@tauri-apps/api/mocks";
import { app } from "./state.svelte";
import type { Tab } from "./types";

const TABS: Tab[] = ["home", "hearthstone", "rocket-league", "league-of-legends", "settings"];

// A fake backend for `pnpm dev` in a plain browser, to look at the window
// without Tauri. Open /?mock&tab=home|hearthstone|rocket-league|league-of-legends|settings
export function install() {
  const tab = new URLSearchParams(location.search).get("tab");
  // `app` is a module-level singleton created before this dynamic import
  // resolves, so its initial tab already came from localStorage: setting
  // localStorage alone would be one page load too late for a screenshot.
  if (tab && (TABS as string[]).includes(tab)) app.setTab(tab as Tab);
  const now = Date.now();
  const iso = (minAgo: number) => new Date(now - minAgo * 60000).toISOString();

  mockIPC(
    (cmd) => {
      switch (cmd) {
        case "get_state":
          return {
            servers: [{ id: "s1", url: "https://kfire.guilde-ke.fr", org_name: "Knights of Eternity", status_override: "inherit" }],
            global_status: "online",
            logged_in: true,
            games_count: 10871,
            running: [{ slug: "hearthstone", name: "Hearthstone" }],
            ignored: [{ server_id: "s1", slug: "fps-monitor", name: "FPS Monitor" }],
            expired_server_url: null,
          };
        case "hs_status":
          return {
            supported: true, enabled: true,
            config_path: "C:\\Users\\djam\\AppData\\Local\\Blizzard\\Hearthstone\\log.config",
            config_block: "[Power]\nLogLevel=1\nFilePrinting=true",
            install_dir: "C:\\Battle.net\\Hearthstone",
            last_match: { mode: "battlegrounds", result: "win", placement: 1, turns: 18, played_at: iso(12) },
          };
        case "rl_status":
          return {
            supported: true, enabled: false,
            config_path: "C:\\Program Files\\Epic Games\\rocketleague\\TAGame\\Config\\DefaultStatsAPI.ini",
            config_block: "[TAGame.MatchStatsExporter_TA]\nPort=49123\nPacketSendRate=30",
            install_dir: "C:\\Program Files\\Epic Games\\rocketleague",
            player_name: "", last_mismatch: "",
            last_match: { result: "win", player_team: 1, team_blue_score: 1, team_orange_score: 3, team_size: 2, played_at: iso(60 * 26) },
          };
        case "rl_live":
          return { last_mismatch: "", watching: false, socket_connected: false, decoded: 0 };
        case "lol_status":
          return { enabled: false, watching: false };
        case "get_autostart":
          return true;
        case "check_for_update":
          return { current: "0.7.0", latest: "0.7.0", update_available: false, releases_url: "" };
        case "plugin:app|version":
          return "0.7.0";
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
