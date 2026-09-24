<script lang="ts">
  let {
    checked,
    disabled = false,
    label,
    onchange,
  }: { checked: boolean; disabled?: boolean; label: string; onchange: (next: boolean) => void } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  class="switch"
  class:on={checked}
  {disabled}
  onclick={() => onchange(!checked)}
></button>

<style>
  .switch { position: relative; width: 36px; height: 20px; flex-shrink: 0; padding: 0; border: none; border-radius: 10px; background: #2a3140; }
  .switch::after { content: ""; position: absolute; top: 2px; left: 2px; width: 16px; height: 16px; border-radius: 50%; background: #9ca3af; transition: left 0.15s; }
  .switch.on { background: #f97316; }
  .switch.on::after { left: 18px; background: #0b0e14; }
  .switch:disabled { opacity: 0.5; cursor: wait; }
</style>
