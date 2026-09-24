<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n";
</script>

<footer>
  <span>KFIRE{app.appVersion ? ` v${app.appVersion}` : ""}</span>
  {#if app.update?.update_available && app.update.latest}
    · <button type="button" class="link" onclick={() => app.update && openUrl(app.update.releases_url)}>
      {t("foot.available", { v: app.update.latest })}
    </button>
  {:else if app.update?.latest}
    · <span class="ok">{t("foot.upToDate")}</span>
  {/if}
</footer>

<style>
  footer { margin-top: auto; padding-top: 0.5rem; text-align: center; font-size: 0.72rem; color: #4b5563; }
  .ok { color: #22c55e; }
  .link { padding: 0; font-size: 0.72rem; font-weight: 600; color: #f97316; background: transparent; border: none; }
  .link:hover { text-decoration: underline; }
</style>
