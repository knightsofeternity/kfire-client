<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { lang, t } from "$lib/i18n";
  import { rlPip } from "$lib/pips";
  import { rlSummary } from "$lib/matches";
  import GameTab from "$lib/components/GameTab.svelte";

  const fields = ["field.mode", "field.score", "field.goals", "field.assists", "field.saves", "field.shots", "field.demos", "field.duration"] as const;
</script>

<GameTab
  slug="rocket-league"
  name="Rocket League"
  pip={rlPip(app.rl)}
  subtitle={t("rl.subtitle")}
  enabled={app.rl?.enabled ?? false}
  busy={app.rlBusy || (!app.rl?.enabled && !app.rlName.trim())}
  canToggle={!!app.rl?.install_dir}
  onToggle={(next) => app.toggleRl(next)}
  fields={fields.map((f) => t(f))}
  privacy={t("rl.private")}
  error={app.rlError}
  lastMatch={rlSummary(app.rl?.last_match ?? null, app.now, lang)}
  techPath={app.rl?.config_path ?? ""}
  techBlock={app.rl?.config_block ?? ""}
>
  {#snippet settings()}
    <label>
      {t("rl.name")}
      <input type="text" bind:value={app.rlName} onblur={() => app.saveRlName()} placeholder={t("rl.namePlaceholder")} />
    </label>
    {#if !app.rlName.trim()}
      <p class="warning">{t("rl.nameMissing")}</p>
    {/if}
  {/snippet}
  {#snippet status()}
    {#if app.rl && !app.rl.install_dir}
      <p class="warning">{t("game.noInstall", { game: "Rocket League" })}</p>
    {:else if app.rl?.enabled}
      {#if !app.rl.watching}
        <p class="muted small">{t("rl.notRunning")}</p>
      {:else if !app.rl.socket_connected}
        <p class="warning" role="status">{t("rl.waitSocket")}</p>
      {:else if app.rl.decoded > 0}
        <p class="muted small">{t("rl.connected", { n: app.rl.decoded })}</p>
      {:else}
        <p class="warning" role="status">{t("rl.silent")}</p>
      {/if}
    {/if}
    {#if app.rl?.last_mismatch}
      <p class="warning" role="status">{t("rl.mismatch", { names: app.rl.last_mismatch })}</p>
    {/if}
  {/snippet}
</GameTab>
