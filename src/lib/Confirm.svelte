<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  let trigger = $state("");

  onMount(() => {
    const un = listen("confirm-show", (e) => {
      trigger = String(e.payload ?? "");
    });
    return () => {
      un.then((f) => f());
    };
  });
</script>

<div class="flex h-screen w-screen items-center p-1">
  <div class="flex w-full items-center gap-3 rounded-xl border border-white/10 bg-[rgba(15,17,24,0.95)] px-4 py-3 shadow-[0_16px_48px_rgba(0,0,0,0.6)]">
    <svg class="h-5 w-5 shrink-0 text-[#9DB6FF]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12 3a9 9 0 1 0 9 9"></path>
      <path d="M12 8v5"></path>
      <path d="M12 16.5h.01"></path>
    </svg>
    <div class="min-w-0 flex-1">
      <div class="truncate text-[13.5px] font-medium text-white/90">Insert "{trigger}"?</div>
      <div class="flex items-center gap-2 text-[11.5px] text-white/40">
        <kbd class="rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/60">Enter</kbd> insert
        <kbd class="rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/60">Esc</kbd> cancel
        <span class="text-white/25">any other key keeps typing</span>
      </div>
    </div>
  </div>
</div>
