<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Palette from "$lib/Palette.svelte";
  import Manager from "$lib/Manager.svelte";
  import Confirm from "$lib/Confirm.svelte";

  function windowLabel(): string {
    try {
      return getCurrentWindow().label;
    } catch {
      return "manager";
    }
  }

  const label = windowLabel();
</script>

{#if label === "confirm"}
  <Confirm />
{:else if label === "manager"}
  <Manager />
{:else}
  <Palette />
{/if}
