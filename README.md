# kfire-client

Desktop client for **KFIRE** (Knight FIRE) — an open-source, self-hosted gaming
presence tracker inspired by Xfire. The client lives in your system tray, detects
which game you are playing, and reports it to your organization's
[kfire-server](https://github.com/knightsofeternity/kfire-server) instance.

> **Status: early scaffold.** The tray and process scanner work; server
> communication, offline cache, and OAuth account linking are TODO.

## What it does (target)

- Runs in the **system tray**, ultra light (< 50 MB RAM)
- Scans local processes every ~5 s ([`sysinfo`](https://crates.io/crates/sysinfo))
  and matches them against a games database (seeded from the Discord
  "detectable games" list)
- Pushes `game_started` / `game_stopped` events to the server over WebSocket
  (contract: [kfire-protocol](https://github.com/knightsofeternity/kfire-protocol))
- Queues events in a local SQLite cache when offline
- Minimal UI: login, settings, OAuth account linking (Steam, Battle.net, …),
  connection status

**Platforms:** Windows, macOS, Linux (Ubuntu first).

## Stack

[Tauri v2](https://tauri.app) (Rust) + [Svelte 5](https://svelte.dev) in the webview.

```
src/                 Svelte UI (login, status)
src-tauri/src/lib.rs tray icon + app lifecycle (close = hide to tray)
src-tauri/src/scanner.rs  process scan loop (game detection)
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
Game detections are logged to stdout for now.

## Related repositories

- [kfire-protocol](https://github.com/knightsofeternity/kfire-protocol) — API & WebSocket contract (Apache-2.0)
- [kfire-server](https://github.com/knightsofeternity/kfire-server) — backend + admin web UI (AGPL-3.0)

## License

[MIT](./LICENSE)
