<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { lang, t } from "$lib/i18n";
  import { hsPip } from "$lib/pips";
  import { hsSummary } from "$lib/matches";
  import GameTab from "$lib/components/GameTab.svelte";

  const fields = ["field.mode", "field.result", "field.turns", "field.placement", "field.hero"] as const;
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
  {/snippet}
</GameTab>
