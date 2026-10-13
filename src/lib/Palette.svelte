<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type Snippet, type Status, type TriggerMode, type TypingMode } from "$lib/api";

  type SnippetItem = { kind: "snippet"; id: string; title: string; sub: string; enabled: boolean; sensitive: boolean; triggerMode: TriggerMode; mode?: string | null };
  type ActionItem = { kind: "action"; id: string; title: string; sub: string };
  type Item = SnippetItem | ActionItem;

  type MenuItem = {
    key: string;
    label: string;
    hint?: string;
    kind: "item" | "toggle" | "separator" | "modelabel" | "triggermode" | "danger";
    checked?: boolean;
  };

  let query = $state("");
  let snippets = $state<Snippet[]>([]);
  let status = $state<Status | null>(null);
  let sel = $state(0);
  let inputEl = $state<HTMLInputElement>();
  let listEl = $state<HTMLDivElement>();
  let menu = $state<{ x: number; y: number; id: string } | null>(null);
  let menuIdx = $state(0);
  let confirmDel = $state(false);

  const modeOptions: { label: string; value: TypingMode | null }[] = [
    { label: "Auto", value: null },
    { label: "Instant", value: "instant" },
    { label: "Fast", value: "fast" },
    { label: "Human", value: "human" },
    { label: "Slow", value: "slow" }
  ];

  const triggerModes: { label: string; value: TriggerMode }[] = [
    { label: "Auto", value: "auto" },
    { label: "Confirm", value: "confirm" },
    { label: "Manual", value: "manual" }
  ];

  function preview(t: string): string {
    const s = t
      .replace(/\{tab\}/gi, "→ ")
      .replace(/\{enter\}/gi, "⏎ ")
      .replace(/\s+/g, " ")
      .trim();
    return s.length > 70 ? s.slice(0, 70) + "…" : s;
  }

  const acts = $derived.by((): ActionItem[] => {
    if (!status) return [];
    const q = query.trim().toLowerCase();
    const all: ActionItem[] = [
      { kind: "action", id: "new", title: "New snippet…", sub: "Open the editor on a fresh trigger" },
      { kind: "action", id: "settings", title: "Open KeyFlow settings", sub: "Snippets, typing modes and import" },
      { kind: "action", id: "import", title: "Import FastKeys rules", sub: "Backup and import live in the same tab" },
      {
        kind: "action",
        id: "pause",
        title: status.paused ? "Resume text expansion" : "Pause text expansion",
        sub: status.paused ? "Expansion is paused" : status.snippet_count + " snippets active"
      },
      { kind: "action", id: "reload", title: "Reload configuration", sub: "Re-read config.json from disk" }
    ];
    return all.filter((a) => !q || a.title.toLowerCase().includes(q) || a.sub.toLowerCase().includes(q));
  });

  const snips = $derived.by((): SnippetItem[] => {
    const q = query.trim().toLowerCase();
    return snippets
      .filter((s) => !q || s.trigger.toLowerCase().includes(q) || s.replacement.toLowerCase().includes(q))
      .slice(0, 80)
      .map((s) => ({
        kind: "snippet" as const,
        id: s.id,
        title: s.trigger,
        sub: preview(s.replacement),
        enabled: s.enabled,
        sensitive: s.sensitive === true,
        triggerMode: s.trigger_mode ?? "auto",
        mode: s.mode ?? null
      }));
  });

  const results = $derived([...acts, ...snips]);

  const menuSnippet = $derived(menu ? snippets.find((s) => s.id === (menu as { id: string }).id) ?? null : null);

  const menuItems = $derived.by((): MenuItem[] => {
    const s = menuSnippet;
    if (!s) return [];
    return [
      { key: "type", label: "Type into window", hint: "↵", kind: "item" },
      { key: "copy", label: "Copy to clipboard", hint: "Ctrl+↵", kind: "item" },
      { key: "copyTrigger", label: "Copy trigger", kind: "item" },
      { key: "sep1", label: "", kind: "separator" },
      { key: "edit", label: "Edit snippet…", kind: "item" },
      { key: "duplicate", label: "Duplicate…", kind: "item" },
      { key: "sep2", label: "", kind: "separator" },
      { key: "enabled", label: "Enabled", kind: "toggle", checked: s.enabled },
      { key: "case", label: "Case sensitive", kind: "toggle", checked: s.case_sensitive },
      { key: "sep2b", label: "", kind: "separator" },
      { key: "triggermode", label: "Trigger mode", kind: "triggermode" },
      { key: "wordboundary", label: "Standalone word", kind: "toggle", checked: s.word_boundary },
      { key: "sep3", label: "", kind: "separator" },
      { key: "modelabel", label: "Typing mode", kind: "modelabel" },
      { key: "sep4", label: "", kind: "separator" },
      { key: "delete", label: "Delete…", kind: "danger" }
    ];
  });

  const navItems = $derived(menuItems.filter((i) => i.kind === "item" || i.kind === "toggle" || i.kind === "danger"));

  function navIndexOf(key: string): number {
    return navItems.findIndex((i) => i.key === key);
  }

  async function refresh() {
    try {
      snippets = await api.listSnippets();
      status = await api.getStatus();
      sel = 0;
    } catch (e) {
      // palette opened before backend ready; ignore
    }
  }

  function move(delta: number) {
    if (results.length === 0) return;
    sel = Math.max(0, Math.min(results.length - 1, sel + delta));
  }

  async function activate(item: Item | undefined, copyOnly = false) {
    if (!item) return;
    if (item.kind === "snippet") {
      if (copyOnly) await api.copySnippet(item.id);
      else await api.executeSnippet(item.id);
      return;
    }
    if (item.id === "new") {
      await api.openManager("new");
    } else if (item.id === "pause") {
      status = await api.setPaused(!(status?.paused ?? false));
      sel = 0;
    } else if (item.id === "settings") {
      await api.openManager(null);
    } else if (item.id === "import") {
      await api.openManager("backup");
    } else if (item.id === "reload") {
      status = await api.reloadConfig();
      snippets = await api.listSnippets();
    }
  }

  function closeMenu() {
    menu = null;
    confirmDel = false;
  }

  function placeMenu(x: number, y: number, id: string) {
    menu = {
      x: Math.max(8, Math.min(x, window.innerWidth - 268)),
      y: Math.max(8, Math.min(y, window.innerHeight - 460)),
      id
    };
    menuIdx = 0;
    confirmDel = false;
  }

  function openMenuAtCursor(e: MouseEvent, id: string) {
    e.preventDefault();
    const idx = results.findIndex((r) => r.kind === "snippet" && r.id === id);
    if (idx >= 0) sel = idx;
    placeMenu(e.clientX, e.clientY, id);
  }

  function openMenuForSelection() {
    const idx = Math.min(sel, Math.max(0, results.length - 1));
    const item = results[idx];
    if (!item || item.kind !== "snippet") return;
    const el = listEl ? (listEl.querySelector('[data-idx="' + idx + '"]') as HTMLElement | null) : null;
    const r = el ? el.getBoundingClientRect() : null;
    const x = r ? r.left + 24 : 80;
    const y = r ? r.top + r.height - 4 : 120;
    placeMenu(x, y, item.id);
  }

  async function runMenuAction(key: string) {
    const s = menuSnippet;
    if (!s) return;
    if (key === "type") {
      closeMenu();
      await api.executeSnippet(s.id);
    } else if (key === "copy") {
      closeMenu();
      await api.copySnippet(s.id);
    } else if (key === "copyTrigger") {
      await api.copyText(s.trigger);
      closeMenu();
    } else if (key === "edit") {
      closeMenu();
      await api.openManagerSnippet("edit", s);
    } else if (key === "duplicate") {
      closeMenu();
      await api.openManagerSnippet("copy", s);
    } else if (key === "enabled") {
      snippets = await api.saveSnippet({ ...s, enabled: !s.enabled });
      closeMenu();
    } else if (key === "case") {
      snippets = await api.saveSnippet({ ...s, case_sensitive: !s.case_sensitive });
      closeMenu();
    } else if (key === "wordboundary") {
      snippets = await api.saveSnippet({ ...s, word_boundary: !s.word_boundary });
      closeMenu();
    } else if (key === "delete") {
      confirmDel = true;
    }
  }

  async function doDelete() {
    const s = menuSnippet;
    if (!s) return;
    snippets = await api.deleteSnippet(s.id);
    await refresh();
    closeMenu();
  }

  async function setMode(m: TypingMode | null) {
    const s = menuSnippet;
    if (!s) return;
    snippets = await api.saveSnippet({ ...s, mode: m });
    closeMenu();
  }

  async function setTriggerMode(m: TriggerMode) {
    const s = menuSnippet;
    if (!s) return;
    snippets = await api.saveSnippet({ ...s, trigger_mode: m });
    closeMenu();
  }

  function onKey(e: KeyboardEvent) {
    if (menu) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        menuIdx = Math.min(menuIdx + 1, Math.max(0, navItems.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        menuIdx = Math.max(menuIdx - 1, 0);
      } else if (e.key === "Enter") {
        e.preventDefault();
        const it = navItems[menuIdx];
        if (it) runMenuAction(it.key);
      } else if (e.key === "Escape") {
        e.preventDefault();
        if (confirmDel) confirmDel = false;
        else closeMenu();
      }
      return;
    }
    if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
      e.preventDefault();
      openMenuForSelection();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      activate(results[sel], e.ctrlKey || e.metaKey);
    } else if ((e.code === "KeyN" || e.key.toLowerCase() === "n") && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      api.openManager("new");
    } else if (e.key === "Escape") {
      e.preventDefault();
      api.hidePalette();
    }
  }

  onMount(() => {
    refresh();
    const un = listen("palette-opened", () => {
      query = "";
      sel = 0;
      menu = null;
      confirmDel = false;
      refresh();
      setTimeout(() => inputEl?.focus(), 40);
    });
    setTimeout(() => inputEl?.focus(), 80);
    return () => {
      un.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div
  class="relative flex h-screen w-screen flex-col overflow-hidden rounded-2xl border border-white/10 bg-[rgba(15,17,24,0.72)] shadow-[0_24px_80px_rgba(0,0,0,0.55)]"
  oncontextmenu={(e) => e.preventDefault()}
>
  <div class="flex items-center gap-3 px-5 pt-4 pb-3">
    <svg class="h-[18px] w-[18px] shrink-0 text-white/35" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
      <circle cx="11" cy="11" r="7"></circle>
      <path d="m20.5 20.5-4-4"></path>
    </svg>
    <input
      bind:this={inputEl}
      bind:value={query}
      oninput={() => {
        closeMenu();
        sel = 0;
      }}
      placeholder="Search snippets or actions…"
      spellcheck="false"
      autocomplete="off"
      class="w-full bg-transparent text-[17px] font-medium text-white/90 outline-none placeholder:text-white/30"
    />
    {#if status?.paused}
      <span class="shrink-0 rounded-md bg-amber-400/15 px-2 py-0.5 text-[11px] font-semibold text-amber-300">PAUSED</span>
    {/if}
  </div>

  <div bind:this={listEl} class="mx-2 min-h-0 flex-1 overflow-y-auto">
    {#if acts.length > 0}
      <div class="px-3 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-white/30">Actions</div>
      {#each acts as item, i (item.kind + item.id)}
        <button
          type="button"
          onmouseenter={() => (sel = i)}
          onclick={() => activate(item)}
          class={["flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left", i === sel ? "bg-white/8" : "hover:bg-white/5"].join(" ")}
        >
          <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white/8 text-white/60">
            {#if item.id === "new"}
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"></path></svg>
            {:else if item.id === "settings"}
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="3"></circle><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1"></path></svg>
            {:else if item.id === "import"}
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12M7 10l5 5 5-5M5 21h14"></path></svg>
            {:else if item.id === "pause"}
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M9 5v14M15 5v14"></path></svg>
            {:else}
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5"></path></svg>
            {/if}
          </span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[14.5px] font-medium text-white/90">{item.title}</span>
            <span class="block truncate text-[12px] text-white/35">{item.sub}</span>
          </span>
        </button>
      {/each}
    {/if}

    {#if snips.length > 0}
      <div class="px-3 pb-1 pt-3 text-[11px] font-semibold uppercase tracking-[0.16em] text-white/30">Snippets</div>
      {#each snips as item, i (item.kind + item.id)}
        <button
          type="button"
          data-idx={acts.length + i}
          onmouseenter={() => (sel = acts.length + i)}
          onclick={() => activate(item)}
          oncontextmenu={(e) => openMenuAtCursor(e, item.id)}
          class={["flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left", acts.length + i === sel ? "bg-white/8" : "hover:bg-white/5"].join(" ")}
        >
          <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-[#5B8CFF]/15 text-[#9DB6FF]">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M4 8h9M4 12h6M4 16h9"></path>
              <path d="m16 6 3 3-3 3"></path>
            </svg>
          </span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[14.5px] font-medium text-white/90">{item.title}</span>
            <span class="flex min-w-0 items-center gap-1.5 text-[12px] text-white/35">
              {#if item.sensitive}
                <svg class="h-3 w-3 shrink-0 text-white/30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M7 11V8a5 5 0 0 1 10 0v3"></path><rect x="5" y="11" width="14" height="9" rx="2"></rect></svg>
              {/if}
              <span class={["min-w-0 truncate", item.sensitive ? "select-none blur-[5px] opacity-60" : ""].join(" ")}>{item.sub}</span>
            </span>
          </span>
          {#if item.triggerMode === "confirm"}
            <span class="shrink-0 rounded-md bg-amber-400/15 px-1.5 py-0.5 text-[10.5px] font-medium uppercase tracking-wide text-amber-300">ask</span>
          {:else if item.triggerMode === "manual"}
            <span class="shrink-0 rounded-md bg-white/10 px-1.5 py-0.5 text-[10.5px] font-medium uppercase tracking-wide text-white/45">manual</span>
          {/if}
          <span class="shrink-0 rounded-md bg-white/5 px-1.5 py-0.5 text-[10.5px] font-medium uppercase tracking-wide text-white/40">
            {item.enabled ? (item.mode ?? "auto") : "off"}
          </span>
        </button>
      {/each}
    {/if}

    {#if results.length === 0}
      <div class="px-3 py-14 text-center text-[13px] text-white/30">No matches — add snippets in settings</div>
    {/if}
  </div>

  <div class="flex items-center gap-4 border-t border-white/5 bg-black/20 px-5 py-2.5 text-[12px] text-white/35">
    <span><kbd class="rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/60">↑↓</kbd> navigate</span>
    <span><kbd class="rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/60">↵</kbd> type</span>
    <span><kbd class="rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/60">Ctrl+↵</kbd> copy</span>
    <span><kbd class="rounded-md bg-white/10 px-1.5 py-0.5 text-[11px] text-white/60">Ctrl+N</kbd> new</span>
    <span class="text-white/30">right-click = options</span>
    <span class="ml-auto">{snippets.length} snippets</span>
  </div>

  {#if menu && menuSnippet}
    <div
      class="fixed inset-0 z-40"
      onclick={closeMenu}
      oncontextmenu={(e) => {
        e.preventDefault();
        closeMenu();
      }}
    ></div>
    <div
      class="absolute z-50 w-[260px] overflow-y-auto rounded-xl border border-white/10 bg-[rgba(20,22,30,0.97)] py-1.5 shadow-[0_18px_48px_rgba(0,0,0,0.65)]"
      style={"left: " + menu.x + "px; top: " + menu.y + "px; max-height: " + Math.max(180, window.innerHeight - menu.y - 14) + "px"}
      oncontextmenu={(e) => e.preventDefault()}
    >
      {#each menuItems as mi (mi.key)}
        {#if mi.kind === "separator"}
          <div class="my-[3px] border-t border-white/5"></div>
        {:else if mi.kind === "modelabel"}
          <div class="px-3 pb-1 pt-1 text-[10.5px] font-semibold uppercase tracking-[0.14em] text-white/30">{mi.label}</div>
          <div class="flex gap-1 px-2 pb-1">
            {#each modeOptions as mo (mo.label)}
              <button
                type="button"
                onclick={() => setMode(mo.value)}
                class={["flex-1 rounded-md border px-1 py-1 text-[11px]", (menuSnippet.mode ?? null) === mo.value ? "border-[#5B8CFF]/60 bg-[#5B8CFF]/20 text-white" : "border-white/5 bg-white/5 text-white/50 hover:bg-white/10"].join(" ")}
              >{mo.label}</button>
            {/each}
          </div>
        {:else if mi.kind === "triggermode"}
          <div class="px-3 pb-1 pt-1 text-[10.5px] font-semibold uppercase tracking-[0.14em] text-white/30">{mi.label}</div>
          <div class="flex gap-1 px-2 pb-1">
            {#each triggerModes as tm (tm.value)}
              <button
                type="button"
                onclick={() => setTriggerMode(tm.value)}
                class={["flex-1 rounded-md border px-1 py-1 text-[10.5px]", (menuSnippet.trigger_mode ?? "auto") === tm.value ? "border-[#5B8CFF]/60 bg-[#5B8CFF]/20 text-white" : "border-white/5 bg-white/5 text-white/50 hover:bg-white/10"].join(" ")}
              >{tm.label}</button>
            {/each}
          </div>
        {:else if mi.kind === "danger"}
          <button
            type="button"
            onmouseenter={() => (menuIdx = navIndexOf(mi.key))}
            onclick={() => (confirmDel = true)}
            class={["flex w-full items-center px-3 py-[5px] text-left text-[13px]", menuIdx === navIndexOf(mi.key) ? "bg-red-500/15" : ""].join(" ")}
          >
            <span class="text-red-400">{mi.label}</span>
          </button>
        {:else}
          <button
            type="button"
            onmouseenter={() => (menuIdx = navIndexOf(mi.key))}
            onclick={() => runMenuAction(mi.key)}
            class={["flex w-full items-center justify-between gap-3 px-3 py-[5px] text-left text-[13px]", menuIdx === navIndexOf(mi.key) ? "bg-white/10" : "hover:bg-white/5"].join(" ")}
          >
            <span class="flex items-center gap-2">
              {#if mi.kind === "toggle"}
                <span class={["inline-block h-3.5 w-3.5 rounded-[4px] border text-center text-[10px] leading-[13px]", mi.checked ? "border-[#5B8CFF] bg-[#5B8CFF]/30 text-white" : "border-white/20 text-transparent"].join(" ")}>✓</span>
              {/if}
              <span class="text-white/85">{mi.label}</span>
            </span>
            {#if mi.hint}<span class="text-[11px] text-white/30">{mi.hint}</span>{/if}
          </button>
        {/if}
      {/each}
      {#if confirmDel}
        <div class="mt-1 border-t border-white/10 bg-red-500/10 px-3 py-2">
          <div class="mb-1.5 text-[12px] text-white/70">Delete "{menuSnippet.trigger}"?</div>
          <div class="flex gap-2">
            <button type="button" onclick={doDelete} class="rounded-md bg-red-500/80 px-2.5 py-1 text-[12px] font-medium text-white hover:bg-red-500">Delete</button>
            <button type="button" onclick={() => (confirmDel = false)} class="rounded-md bg-white/5 px-2.5 py-1 text-[12px] text-white/60 hover:bg-white/10">Cancel</button>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>
