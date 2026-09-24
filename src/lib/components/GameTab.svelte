<script lang="ts">
  import type { Snippet } from "svelte";
  import Switch from "./Switch.svelte";
  import { t } from "$lib/i18n";
  import type { Pip } from "$lib/types";
  import type { Summary } from "$lib/matches";

  let {
    slug,
    name,
    pip,
    subtitle,
    enabled,
    busy,
    canToggle = true,
    onToggle,
    fields,
    privacy,
    error = "",
    lastMatch = null,
    techPath = "",
    techBlock = "",
    settings,
    status,
  }: {
    slug: string;
    name: string;
    pip: Pip;
    subtitle: string;
    enabled: boolean;
    busy: boolean;
    canToggle?: boolean;
    onToggle: (next: boolean) => void;
    fields: string[];
    privacy: string;
    error?: string;
    lastMatch?: Summary | null;
    techPath?: string;
    techBlock?: string;
    settings?: Snippet;
    status?: Snippet;
  } = $props();

  const pipKey = { on: "game.pip.on", todo: "game.pip.todo", off: "game.pip.off" } as const;
</script>

<section class="stack">
  <header class="row">
    <img class="icon" src="/games/{slug}.png" alt="" />
    <h1 class="grow ellipsis">{name}</h1>
    <span class="pill {pip}">{t(pipKey[pip])}</span>
  </header>

  {@render settings?.()}

  {#if canToggle}
    <div class="card row">
      <div class="grow">
        <b>{t("game.track")}</b>
        <div class="muted small">{subtitle}</div>
      </div>
      <Switch checked={enabled} disabled={busy} label={t("game.track")} onchange={onToggle} />
    </div>
  {/if}

  {@render status?.()}

  <h2>{t("game.sent")}</h2>
  <ul class="sent">
    {#each fields as f (f)}<li>{f}</li>{/each}
  </ul>
  <p class="lock"><span aria-hidden="true">🔒</span><span>{privacy}</span></p>

  {#if lastMatch}
    <h2>{t("game.lastMatch")}</h2>
    <div class="card row">
      <div class="grow">
        <div class="ellipsis">{lastMatch.title}</div>
        <div class="muted small">{lastMatch.detail}</div>
      </div>
    </div>
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#if techPath}
    <details>
      <summary>{t("game.tech")}</summary>
      <p class="muted small">{t("game.techFile")}</p>
      <pre class="tech">{techPath}</pre>
      <p class="muted small">{t("game.techContent")}</p>
      <pre class="tech">{techBlock}</pre>
    </details>
  {/if}
</section>

<style>
  .icon { width: 30px; height: 30px; border-radius: 8px; }
  .pill { font-size: 0.66rem; font-weight: 700; padding: 3px 8px; border-radius: 99px; text-transform: uppercase; letter-spacing: 0.05em; flex-shrink: 0; }
  .pill.on { color: #22c55e; background: rgba(34, 197, 94, 0.12); }
  .pill.todo { color: #f59e0b; background: rgba(245, 158, 11, 0.12); }
  .pill.off { color: #9ca3af; background: rgba(156, 163, 175, 0.12); }
  ul.sent { margin: 0; padding: 0; list-style: none; display: flex; flex-wrap: wrap; gap: 5px; }
  ul.sent li { font-size: 0.72rem; padding: 3px 8px; border-radius: 6px; background: #0b0e14; border: 1px solid #2a3140; }
  .lock { margin: 0; display: flex; gap: 6px; align-items: flex-start; color: #9ca3af; font-size: 0.75rem; line-height: 1.35; }
  details { font-size: 0.75rem; color: #6b7280; }
  summary { cursor: pointer; color: #9ca3af; }
</style>
