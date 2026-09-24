<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n";
  import Footer from "./Footer.svelte";
</script>

<main class="stack full">
  <h1 class="brand">KFIRE</h1>
  {#if app.pairing}
    <section class="stack">
      <p>{t("pair.opened")}</p>
      <p class="muted">{t("pair.ifNot")}</p>
      <a class="url" href={app.pairing.verification_url} target="_blank">{app.pairing.verification_url}</a>
      <p class="muted">{t("pair.approve")}</p>
      <p class="code">{app.pairing.user_code}</p>
      <p class="muted small">{t("pair.waiting")}</p>
      <button class="btn ghost" onclick={() => app.cancel()}>{t("common.cancel")}</button>
    </section>
  {:else}
    <form class="stack" onsubmit={(e) => app.startLink(e)}>
      {#if app.expiredUrl}
        <p class="error">{t("link.expired", { url: app.expiredUrl })}</p>
      {:else}
        <p class="muted">{t("link.intro")}</p>
      {/if}
      <label>
        {t("link.address")}
        <input type="url" placeholder="https://kfire.example.org" bind:value={app.serverUrl} required />
      </label>
      {#if app.error}<p class="error">{app.error}</p>{/if}
      <button class="btn" type="submit" disabled={app.linking}>
        {app.linking ? t("link.opening") : t("link.submit")}
      </button>
    </form>
  {/if}
  <Footer />
</main>

<style>
  .full { height: 100vh; padding: 1.5rem; }
  .code { font-size: 1.6rem; font-weight: 700; letter-spacing: 0.18em; color: #f97316; text-align: center; margin: 0.2rem 0; }
  .url { color: #fb923c; font-size: 0.85rem; word-break: break-all; }
  p { margin: 0; }
</style>
