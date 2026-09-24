<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app } from "$lib/state.svelte";
  import { lang, t } from "$lib/i18n";
  import { hsPip } from "$lib/pips";
  import { hsSummary } from "$lib/matches";
  import GameTab from "$lib/components/GameTab.svelte";
  import Switch from "$lib/components/Switch.svelte";

  const HDT_URL = "https://hsreplay.net/downloads/";
  const base = ["field.mode", "field.result", "field.turns", "field.placement", "field.hero"] as const;
  const fields = $derived([...base, ...(app.hs?.hdt_available && app.hs.hdt_enabled ? (["field.rating"] as const) : [])]);
</script>

<GameTab
  slug="hearthstone"
  name="Hearthstone"
  pip={hsPip(app.hs)}
  subtitle={t("hs.subtitle")}
  enabled={app.hs?.enabled ?? false}
  busy={app.hsBusy}
  canToggle={app.hs?.supported ?? false}
  onToggle={(next) => app.toggleHs(next)}
  fields={fields.map((f) => t(f))}
  privacy={t("hs.private")}
  error={app.hsError}
  lastMatch={hsSummary(app.hs?.last_match ?? null, app.now, lang)}
  techPath={app.hs?.config_path ?? ""}
  techBlock={app.hs?.config_block ?? ""}
>
  {#snippet status()}
    {#if app.hs && !app.hs.supported}
      <p class="muted">{t("hs.linux")}</p>
    {:else if app.hs && !app.hs.install_dir}
      <p class="warning">{t("game.noInstall", { game: "Hearthstone" })}</p>
    {/if}
    {#if app.hs?.supported}
      {#if app.hs.hdt_available}
        <div class="card row">
          <div class="grow">
            <b>{t("hs.hdt")}</b>
            <div class="muted small">{t("hs.hdtSub")}</div>
          </div>
          <Switch checked={app.hs.hdt_enabled} disabled={app.hdtBusy} label={t("hs.hdt")} onchange={(next) => app.toggleHdt(next)} />
        </div>
        {#if app.hs.hdt_enabled && app.hs.last_match?.rating_missing === true}
          <p class="warning">{t("hs.hdtNotFound")} <button type="button" class="linkish" onclick={() => openUrl(HDT_URL)}>{t("hs.hdtGet")}</button></p>
        {/if}
      {:else}
        <p class="muted small">{t("hs.hdtMissing")} <button type="button" class="linkish" onclick={() => openUrl(HDT_URL)}>{t("hs.hdtGet")}</button></p>
      {/if}
    {/if}
  {/snippet}
</GameTab>

<style>
  .linkish { padding: 0; border: none; background: none; color: #fb923c; text-decoration: underline; font-size: inherit; }
</style>
