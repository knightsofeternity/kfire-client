# kfire-client

Desktop client for **KFIRE** (Knight FIRE) — an open-source, self-hosted gaming
presence tracker inspired by Xfire. The client lives in your system tray, detects
which game you are playing, and reports it to your organization's
[kfire-server](https://github.com/knightsofeternity/kfire-server) instance.

> **Status: functional.** Login, games catalog sync, process detection,
> WebSocket presence push with reconnection, and the offline queue all work.
> OAuth account linking is TODO.

## What it does

- Runs in the **system tray**, ultra light
- Signs in to your org's server (device-bound session, auto-resumed at startup)
- Downloads the games catalog (~10k games) and caches it in **SQLite**
- Scans local processes every 5 s ([`sysinfo`](https://crates.io/crates/sysinfo))
  and matches them against the catalog (exe basename → game)
- Pushes `game_started` / `game_stopped` over WebSocket
  (contract: [kfire-protocol](https://github.com/knightsofeternity/kfire-protocol)),
  with heartbeat, exponential-backoff reconnection, re-announce after
  reconnect, and an **offline queue**: detections made while disconnected are
  stored in SQLite and flushed on reconnect
- Minimal UI: login, live connection status, currently detected games

TODO: OAuth account linking (Steam, Battle.net, …), refresh token in the OS
keychain instead of SQLite.

**Platforms:** Windows, macOS, Linux (Ubuntu first).

## Stack

[Tauri v2](https://tauri.app) (Rust) + [Svelte 5](https://svelte.dev) in the webview.

```
src/                      Svelte UI (login, status, detected games)
src-tauri/src/lib.rs      app wiring: state, commands, tray, event routing
src-tauri/src/api.rs      REST client (login, refresh, games download)
src-tauri/src/db.rs       SQLite cache (settings, catalog, offline queue)
src-tauri/src/scanner.rs  process scan loop (exe → game matching)
src-tauri/src/ws.rs       WebSocket task (hello, heartbeat, backoff, drain)
```

## Development

Prerequisites: [Rust](https://rustup.rs), Node ≥ 20, pnpm, and the
[Tauri Linux system deps](https://tauri.app/start/prerequisites/#linux)
(`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`,
`librsvg2-dev`, `libxdo-dev`) on Ubuntu.

```bash
pnpm install
pnpm tauri dev      # run with hot reload
pnpm tauri build    # production bundles (deb/AppImage/msi/dmg)
```

The window starts hidden — open it from the tray icon (**Show KFIRE**).

**Integration test** (requires a running kfire-server):

```bash
KFIRE_TEST_SERVER=http://127.0.0.1:8091 cargo test --test integration
```

## Related repositories

- [kfire-protocol](https://github.com/knightsofeternity/kfire-protocol) — API & WebSocket contract (Apache-2.0)
- [kfire-server](https://github.com/knightsofeternity/kfire-server) — backend + admin web UI (AGPL-3.0)

## License

[MIT](./LICENSE)
