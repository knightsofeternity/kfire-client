<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n";
  import { lolPip } from "$lib/pips";
  import GameTab from "$lib/components/GameTab.svelte";

  const fields = ["field.champion", "field.level", "field.kda", "field.cs", "field.gold", "field.time"] as const;
</script>

<GameTab
  slug="league-of-legends"
  name="League of Legends"
  pip={lolPip(app.lol)}
  subtitle={t("lol.subtitle")}
  enabled={app.lol?.enabled ?? false}
  busy={app.lolBusy}
  onToggle={(next) => app.toggleLol(next)}
  fields={fields.map((f) => t(f))}
  privacy={t("lol.private")}
  error={app.lolError}
>
  {#snippet status()}
    {#if app.lol?.enabled}
      <p class="muted small">{app.lol.watching ? t("lol.watching") : t("lol.idle")}</p>
    {:else}
      <p class="muted small">{t("lol.noSetup")}</p>
    {/if}
  {/snippet}
</GameTab>
