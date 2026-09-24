<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/state.svelte";
  import Rail from "$lib/components/Rail.svelte";
  import Link from "$lib/components/Link.svelte";
  import Footer from "$lib/components/Footer.svelte";
  import Home from "$lib/tabs/Home.svelte";
  import Settings from "$lib/tabs/Settings.svelte";
  import Hearthstone from "$lib/tabs/Hearthstone.svelte";
  import RocketLeague from "$lib/tabs/RocketLeague.svelte";
  import LeagueOfLegends from "$lib/tabs/LeagueOfLegends.svelte";

  onMount(() => {
    let cleanup = () => {};
    (async () => {
      // Dev only, never in a build: `pnpm dev` then open /?mock&tab=<tab>.
      if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock")) {
        (await import("$lib/devmock")).install();
      }
      cleanup = app.init();
    })();
    return () => cleanup();
  });
</script>

{#if !app.loaded}
  <!-- Nothing until the first state arrives: no flash of the link screen. -->
{:else if app.pairing || app.servers.length === 0}
  <Link />
{:else}
  <div class="shell">
    <Rail />
    <main>
      {#if app.tab === "home"}<Home />
      {:else if app.tab === "hearthstone"}<Hearthstone />
      {:else if app.tab === "rocket-league"}<RocketLeague />
      {:else if app.tab === "league-of-legends"}<LeagueOfLegends />
      {:else}<Settings />{/if}
      <Footer />
    </main>
  </div>
{/if}

<style>
  .shell { display: flex; height: 100vh; }
  main { flex: 1; min-width: 0; display: flex; flex-direction: column; padding: 16px 16px 10px; overflow-y: auto; }
</style>
