<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n";
  import { hsPip, lolPip, rlPip } from "$lib/pips";
  import type { Pip, Tab } from "$lib/types";

  const games: { tab: Tab; name: string; pip: () => Pip }[] = [
    { tab: "hearthstone", name: "Hearthstone", pip: () => hsPip(app.hs) },
    { tab: "rocket-league", name: "Rocket League", pip: () => rlPip(app.rl) },
    { tab: "league-of-legends", name: "League of Legends", pip: () => lolPip(app.lol) },
  ];
</script>

<nav aria-label="KFIRE">
  <img class="logo" src="/favicon.png" alt="" />
  <button type="button" class="tab" class:active={app.tab === "home"} title={t("tab.home")} aria-label={t("tab.home")} onclick={() => app.setTab("home")}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 11l9-7 9 7" /><path d="M5 10v10h14V10" /><path d="M10 20v-6h4v6" /></svg>
  </button>
  <div class="sep"></div>
  {#each games as g (g.tab)}
    <button type="button" class="tab" class:active={app.tab === g.tab} title={g.name} aria-label={g.name} onclick={() => app.setTab(g.tab)}>
      <img src="/games/{g.tab}.png" alt="" />
      <span class="pip {g.pip()}"></span>
    </button>
  {/each}
  <div class="spacer"></div>
  <button type="button" class="tab" class:active={app.tab === "settings"} title={t("tab.settings")} aria-label={t("tab.settings")} onclick={() => app.setTab("settings")}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" /></svg>
  </button>
</nav>

<style>
  nav { width: 56px; flex-shrink: 0; height: 100vh; background: #080a0f; border-right: 1px solid #1c222d; display: flex; flex-direction: column; align-items: center; padding: 10px 0; gap: 6px; }
  .logo { width: 30px; height: 30px; margin-bottom: 8px; }
  .sep { width: 28px; height: 1px; background: #1c222d; margin: 4px 0; }
  .spacer { flex: 1; }
  .tab { position: relative; width: 40px; height: 40px; padding: 0; border: none; border-radius: 12px; display: grid; place-items: center; color: #6b7280; background: transparent; }
  .tab:hover { background: #151a23; color: #e5e7eb; }
  .tab.active { background: #151a23; color: #f97316; }
  .tab.active::before { content: ""; position: absolute; left: -8px; top: 9px; width: 4px; height: 22px; border-radius: 0 4px 4px 0; background: #f97316; }
  .tab img { width: 28px; height: 28px; border-radius: 7px; }
  .tab:not(.active) img { filter: saturate(0.55) brightness(0.85); }
  .tab svg { width: 22px; height: 22px; }
  .pip { position: absolute; right: 2px; bottom: 2px; width: 11px; height: 11px; border-radius: 50%; border: 2px solid #080a0f; }
  .pip.on { background: #22c55e; }
  .pip.off { background: #4b5563; }
  .pip.todo { background: #f59e0b; }
</style>
