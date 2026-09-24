<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { lang, t, type Key } from "$lib/i18n";
  import { hsPip, lolPip, rlPip } from "$lib/pips";
  import type { Pip } from "$lib/types";

  const statuses = ["online", "invisible", "offline"] as const;
  const modulePip: Record<string, () => Pip> = {
    hearthstone: () => hsPip(app.hs),
    "rocket-league": () => rlPip(app.rl),
    "league-of-legends": () => lolPip(app.lol),
  };
  const nf = new Intl.NumberFormat(lang);
</script>

<section class="stack">
  <h1 class="brand">KFIRE</h1>

  <h2>{t("home.status")}</h2>
  <div class="seg" role="radiogroup" aria-label={t("home.status")}>
    {#each statuses as s (s)}
      <button type="button" role="radio" aria-checked={app.globalStatus === s} class:on={app.globalStatus === s} onclick={() => app.setGlobal(s)}>
        {t(`status.${s}` as Key)}
      </button>
    {/each}
  </div>

  {#each app.servers as srv (srv.id)}
    <div class="card row">
      <span class="dot {app.statusOf(srv.id).status}"></span>
      <div class="grow">
        <b class="ellipsis">{srv.org_name || srv.url}</b>
        <div class="muted small">{app.statusLabel(srv.id)}</div>
      </div>
    </div>
  {/each}

  <h2>{t("home.nowPlaying")}</h2>
  {#if app.running.length === 0}
    <p class="muted">{t("home.noGame")}</p>
  {:else}
    {#each app.running as game (game.slug)}
      {@const pip = modulePip[game.slug]?.()}
      <div class="card row">
        {#if pip}<img class="icon" src="/games/{game.slug}.png" alt="" />{/if}
        <div class="grow">
          <b class="ellipsis">{game.name}</b>
          {#if pip}<div class="muted small">{pip === "on" ? t("home.tracked") : t("home.untracked")}</div>{/if}
        </div>
        <button class="mini" onclick={() => app.stopGame(game.slug)}>{t("home.stop")}</button>
        <button class="mini" title={t("home.ignoreTitle")} onclick={() => app.ignoreGame(game.slug, true)}>{t("home.ignore")}</button>
      </div>
    {/each}
  {/if}

  <p class="muted small">{t("home.gamesCount", { n: nf.format(app.gamesCount) })}</p>
  <p class="muted small">{t("foot.tray")}</p>
</section>

<style>
  .seg { display: flex; padding: 2px; background: #0b0e14; border: 1px solid #2a3140; border-radius: 8px; }
  .seg button { flex: 1; padding: 5px 0; font-size: 0.78rem; color: #9ca3af; background: transparent; border: none; border-radius: 6px; }
  .seg button.on { background: #f97316; color: #0b0e14; font-weight: 700; }
  .icon { width: 32px; height: 32px; border-radius: 8px; }
</style>
