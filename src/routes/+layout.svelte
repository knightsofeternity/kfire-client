<script lang="ts">
  import "$lib/theme.css";
  import { lang } from "$lib/i18n";
  let { children } = $props();

  $effect(() => {
    document.documentElement.lang = lang;
  });
</script>

{@render children()}
