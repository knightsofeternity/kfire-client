<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { lang, t } from "$lib/i18n";
  import { wowPip } from "$lib/pips";
  import { ago } from "$lib/matches";
  import GameTab from "$lib/components/GameTab.svelte";
  import type { WowEdition } from "$lib/types";

  const fields = ["field.played", "field.level", "field.class", "field.realm"] as const;
  const stateKey = {
    installed: "wow.state.installed",
    pending: "wow.state.pending",
    foreign: "wow.state.foreign",
    off: "wow.state.off",
  } as const;

  function savedLine(e: WowEdition): string {
    return e.saved_at ? t("wow.saved", { when: ago(e.saved_at, app.now, lang) }) : t("wow.neverSaved");
  }
</script>

<GameTab
  slug="world-of-warcraft"
  name="World of Warcraft"
  pip={wowPip(app.wow)}
  subtitle={t("wow.subtitle")}
  toggleLabel={t("wow.toggle")}
  enabled={app.wow?.enabled ?? false}
  busy={app.wowBusy}
  canToggle={(app.wow?.supported ?? false) && (app.wow?.editions.length ?? 0) > 0}
  onToggle={(next) => app.toggleWow(next)}
  fields={fields.map((f) => t(f))}
  privacy={t("wow.private")}
  error={app.wowError || app.wow?.error || ""}
>
  {#snippet status()}
    {#if app.wow && !app.wow.supported}
      <p class="muted small">{t("wow.unsupported")}</p>
    {:else if app.wow && app.wow.editions.length === 0}
      <p class="warning">{t("game.noInstall", { game: "World of Warcraft" })}</p>
    {:else if app.wow}
      {#each app.wow.editions as e (e.dir)}
        <div class="card">
          <div class="row">
            <b class="grow">{e.label}</b>
            <span class="muted small">{t(stateKey[e.state])}</span>
          </div>
          {#if e.state === "foreign"}
            <p class="warning">{t("wow.foreign")}</p>
          {:else if app.wow.enabled}
            <div class="muted small">{savedLine(e)}</div>
            {#if e.characters > 0}
              <div class="muted small">{t("wow.characters", { n: e.characters })}</div>
            {/if}
          {/if}
        </div>
      {/each}
      {#if app.wow.enabled}<p class="muted small">{t("wow.help")}</p>{/if}
    {/if}
  {/snippet}
</GameTab>
