import type { Pip } from "./types";

// The state dot on each game icon. "todo" wins over "on": an enabled game that
// cannot work as set up is exactly what the dot must point at.

export function hsPip(
  s: { supported: boolean; enabled: boolean; install_dir: string | null } | null,
): Pip {
  if (!s || !s.supported) return "off";
  if (s.enabled) return "on";
  return s.install_dir ? "off" : "todo";
}

export function rlPip(
  s: { enabled: boolean; install_dir: string | null; player_name: string; last_mismatch: string } | null,
): Pip {
  if (!s) return "off";
  if (!s.player_name.trim() || !s.install_dir || s.last_mismatch) return "todo";
  return s.enabled ? "on" : "off";
}

export function lolPip(s: { enabled: boolean } | null): Pip {
  return s?.enabled ? "on" : "off";
}
