<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n";
  import Switch from "$lib/components/Switch.svelte";
</script>

<section class="stack">
  <h1>{t("tab.settings")}</h1>

  <h2>{t("set.servers")}</h2>
  {#each app.servers as srv (srv.id)}
    <div class="card row">
      <span class="dot {app.statusOf(srv.id).status}"></span>
      <div class="grow">
        <b class="ellipsis">{srv.org_name || srv.url}</b>
        <div class="muted small ellipsis">{srv.url.replace(/^https?:\/\//, "")}</div>
      </div>
      <select value={srv.status_override} title={t("set.serverStatusTitle")} onchange={(e) => app.setServer(srv.id, e.currentTarget.value)}>
        <option value="inherit">{t("set.useGlobal")}</option>
        <option value="online">{t("status.online")}</option>
        <option value="invisible">{t("status.invisible")}</option>
        <option value="offline">{t("status.offline")}</option>
      </select>
      <button class="mini danger" title={t("set.unlinkTitle")} onclick={() => app.unlink(srv.id)}>{t("set.unlink")}</button>
    </div>
  {/each}

  {#if app.adding}
    <form class="stack" onsubmit={(e) => app.startLink(e)}>
      <label>
        {t("link.address")}
        <input type="url" placeholder="https://kfire.example.org" bind:value={app.serverUrl} required />
      </label>
      {#if app.error}<p class="error">{app.error}</p>{/if}
      <div class="row">
        <button class="btn grow" type="submit" disabled={app.linking}>{app.linking ? t("link.opening") : t("set.link")}</button>
        <button class="btn ghost grow" type="button" onclick={() => app.cancel()}>{t("common.cancel")}</button>
      </div>
    </form>
  {:else}
    <button class="btn ghost" onclick={() => { app.adding = true; app.error = ""; }}>{t("set.add")}</button>
  {/if}

  <h2>{t("set.startup")}</h2>
  <div class="card row">
    <div class="grow">{t("set.autostart")}</div>
    <Switch checked={app.autostart} label={t("set.autostart")} onchange={() => app.toggleAutostart()} />
  </div>

  <h2>{t("set.ignored")}</h2>
  {#if app.ignored.length === 0}
    <p class="muted">{t("set.noIgnored")}</p>
  {:else}
    {#each app.ignored as g (g.server_id + g.slug)}
      <div class="card row">
        <div class="grow ellipsis">{g.name}</div>
        <button class="mini" onclick={() => app.ignoreGame(g.slug, false)}>{t("set.reenable")}</button>
      </div>
    {/each}
  {/if}

  <p class="muted small">{t("foot.tray")}</p>
</section>
