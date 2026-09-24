export type RunningGame = { slug: string; name: string };
export type IgnoredGame = { server_id: string; slug: string; name: string };
export type UiServer = { id: string; url: string; org_name: string; status_override: string };
export type UiState = {
  servers: UiServer[];
  global_status: string;
  logged_in: boolean;
  games_count: number;
  running: RunningGame[];
  ignored: IgnoredGame[];
  expired_server_url: string | null;
};
export type LinkInfo = { user_code: string; verification_url: string };
export type ServerStatus = "disconnected" | "connecting" | "connected" | "logged_out";
export type StatusEvent = { server_id: string; status: ServerStatus; detail: string };
export type UpdateInfo = {
  current: string;
  latest: string | null;
  update_available: boolean;
  releases_url: string;
};

/** A match payload exactly as the client queued it. */
export type MatchPayload = Record<string, unknown>;

export type HsStatus = {
  supported: boolean;
  enabled: boolean;
  config_path: string;
  config_block: string;
  install_dir: string | null;
  last_match: MatchPayload | null;
};
export type RlLive = {
  last_mismatch: string;
  watching: boolean;
  socket_connected: boolean;
  decoded: number;
};
export type RlStatus = {
  supported: boolean;
  enabled: boolean;
  config_path: string;
  config_block: string;
  install_dir: string | null;
  player_name: string;
  last_match: MatchPayload | null;
} & RlLive;
export type LolStatus = { enabled: boolean; watching: boolean };

export type Tab = "home" | "hearthstone" | "rocket-league" | "league-of-legends" | "settings";
export type Pip = "on" | "todo" | "off";
