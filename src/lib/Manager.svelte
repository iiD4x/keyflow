<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type ImportReport, type Snippet, type Status, type TypingMode } from "$lib/api";

  type Tab = "snippets" | "import" | "settings" | "about";

  let tab = $state<Tab>("snippets");
  let snippets = $state<Snippet[]>([]);
  let status = $state<Status | null>(null);
  let filter = $state("");
  let editing = $state<Snippet | null>(null);
  let report = $state<ImportReport | null>(null);
  let importPath = $state("");
  let replaceExisting = $state(false);
  let busy = $state(false);
  let toast = $state<string | null>(null);
  let triggerInput = $state<HTMLInputElement>();
  let replEl = $state<HTMLTextAreaElement>();
  let dragOver = $state(false);
  let comboMods = $state({ ctrl: false, shift: false, alt: false, win: false });
  let comboKey = $state("s");
  let sleepMs = $state(250);
  let history = $state<string[]>([]);
  let histIdx = $state(-1);
  let histTimer: ReturnType<typeof setTimeout> | null = null;
  let lastEditingRef: Snippet | null = null;

  const modes: TypingMode[] = ["instant", "fast", "human", "slow"];
  const tabs: { id: Tab; label: string }[] = [
    { id: "snippets", label: "Snippets" },
    { id: "import", label: "Import" },
    { id: "settings", label: "Settings" },
    { id: "about", label: "About" }
  ];

  const keyChips = [
    { label: "Enter", token: "{enter}" },
    { label: "Tab", token: "{tab}" },
    { label: "Space", token: "{space}" },
    { label: "Backspace", token: "{backspace}" },
    { label: "Delete", token: "{delete}" },
    { label: "Esc", token: "{esc}" },
    { label: "↑", token: "{up}" },
    { label: "↓", token: "{down}" },
    { label: "←", token: "{left}" },
    { label: "→", token: "{right}" },
    { label: "Home", token: "{home}" },
    { label: "End", token: "{end}" },
    { label: "PgUp", token: "{pgup}" },
    { label: "PgDn", token: "{pgdn}" },
    { label: "Ins", token: "{insert}" }
  ];
  const mouseChips = [
    { label: "Left click", token: "{lbutton}" },
    { label: "Right click", token: "{rbutton}" },
    { label: "Middle click", token: "{mbutton}" }
  ];
  const fKeys = Array.from({ length: 24 }, (_, i) => "f" + (i + 1));
  const letters = "abcdefghijklmnopqrstuvwxyz".split("");
  const digits = "0123456789".split("");
  const comboNamed = ["enter", "tab", "space", "esc", "delete", "home", "end", "left", "right", "up", "down", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12"];
  const modDefs: { key: "ctrl" | "shift" | "alt" | "win"; label: string }[] = [
    { key: "ctrl", label: "Ctrl" },
    { key: "shift", label: "Shift" },
    { key: "alt", label: "Alt" },
    { key: "win", label: "Win" }
  ];
  const NAMED_KEYS = new Set(["enter", "return", "tab", "space", "backspace", "bs", "delete", "del", "esc", "escape", "up", "down", "left", "right", "home", "end", "pgup", "pageup", "pgdn", "pagedown", "insert", "ins"]);
  const MOUSE_TOKENS = new Set(["lbutton", "lclick", "rbutton", "rclick", "mbutton", "mclick"]);

  function isValidToken(raw: string): boolean {
    const t = raw.slice(1, -1).trim().toLowerCase();
    if (!t) return false;
    if (t.startsWith("sleep")) {
      const n = t.slice(5).trim().replace(/ms$/, "").trim();
      return /^\d+$/.test(n);
    }
    if (MOUSE_TOKENS.has(t)) return true;
    const parts = t.split("+").map((p) => p.trim()).filter(Boolean);
    if (parts.length < 1) return false;
    const key = parts[parts.length - 1];
    const mods = parts.slice(0, -1);
    const modOk = mods.every((m) => ["ctrl", "control", "shift", "alt", "win", "windows", "meta"].includes(m));
    if (!modOk) return false;
    return NAMED_KEYS.has(key) || /^f([1-9]|1[0-9]|2[0-4])$/.test(key) || /^[a-z0-9]$/.test(key);
  }

  function previewParts(s: string): { text: string; isToken: boolean }[] {
    const out: { text: string; isToken: boolean }[] = [];
    const re = /\{[^{}]*\}/g;
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = re.exec(s)) !== null) {
      if (m.index > last) out.push({ text: s.slice(last, m.index), isToken: false });
      out.push({ text: m[0], isToken: true });
      last = m.index + m[0].length;
    }
    if (last < s.length) out.push({ text: s.slice(last), isToken: false });
    return out;
  }

  function insertAtCaret(text: string, at?: number | null) {
    if (!editing) return;
    const cur = editing.replacement ?? "";
    const el = replEl;
    const pos = typeof at === "number" && at >= 0 && at <= cur.length ? at : el ? el.selectionStart ?? cur.length : cur.length;
    editing.replacement = cur.slice(0, pos) + text + cur.slice(pos);
    const next = pos + text.length;
    if (el) {
      tick().then(() => {
        el.focus();
        el.setSelectionRange(next, next);
      });
    }
  }

  function tokenAtPoint(x: number, y: number): number | null {
    const doc = document as unknown as { caretRangeFromPoint?: (x: number, y: number) => Range | null };
    if (typeof doc.caretRangeFromPoint === "function") {
      const range = doc.caretRangeFromPoint(x, y);
      if (range) return range.startOffset;
    }
    return null;
  }

  function toggleMod(k: "ctrl" | "shift" | "alt" | "win") {
    comboMods[k] = !comboMods[k];
  }

  function insertFKey(ev: Event) {
    const sel = ev.target as HTMLSelectElement;
    const v = sel.value;
    if (v) insertAtCaret("{" + v + "}");
    sel.value = "";
  }

  const comboToken = $derived.by(() => {
    const mods: string[] = [];
    if (comboMods.ctrl) mods.push("ctrl");
    if (comboMods.shift) mods.push("shift");
    if (comboMods.alt) mods.push("alt");
    if (comboMods.win) mods.push("win");
    return "{" + [...mods, comboKey].join("+") + "}";
  });

  function snapshotOf(s: { trigger: string; replacement?: string }): string {
    return JSON.stringify({ t: s.trigger, r: s.replacement ?? "" });
  }

  function recordHistory() {
    if (!editing) return;
    const snap = snapshotOf(editing);
    if (history[histIdx] === snap) return;
    history = history.slice(0, histIdx + 1);
    history.push(snap);
    histIdx = history.length - 1;
    if (history.length > 100) {
      history.shift();
      histIdx -= 1;
    }
  }

  function scheduleHistory() {
    if (histTimer) clearTimeout(histTimer);
    histTimer = setTimeout(() => {
      histTimer = null;
      recordHistory();
    }, 500);
  }

  function applySnapshot(s: string) {
    if (!editing) return;
    const obj = JSON.parse(s) as { t: string; r: string };
    editing.trigger = obj.t;
    editing.replacement = obj.r;
  }

  function undoEdit() {
    if (histTimer) {
      clearTimeout(histTimer);
      histTimer = null;
    }
    recordHistory();
    if (histIdx <= 0) return;
    histIdx -= 1;
    applySnapshot(history[histIdx]);
  }

  function redoEdit() {
    if (histTimer) {
      clearTimeout(histTimer);
      histTimer = null;
    }
    if (histIdx >= history.length - 1) return;
    histIdx += 1;
    applySnapshot(history[histIdx]);
  }

  function onEditorKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;
    const k = e.key.toLowerCase();
    if (k === "z") {
      e.preventDefault();
      if (e.shiftKey) redoEdit();
      else undoEdit();
    } else if (k === "y") {
      e.preventDefault();
      redoEdit();
    }
  }

  const canUndo = $derived(histIdx > 0);
  const canRedo = $derived(histIdx < history.length - 1);

  $effect(() => {
    const cur = editing;
    if (cur === lastEditingRef) return;
    lastEditingRef = cur;
    if (histTimer) {
      clearTimeout(histTimer);
      histTimer = null;
    }
    if (!cur) {
      history = [];
      histIdx = -1;
      return;
    }
    history = [snapshotOf(cur)];
    histIdx = 0;
  });

  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const list = q
      ? snippets.filter((s) => s.trigger.toLowerCase().includes(q) || s.replacement.toLowerCase().includes(q))
      : snippets.slice();
    return list.sort((a, b) => a.trigger.toLowerCase().localeCompare(b.trigger.toLowerCase()));
  });

  function flash(msg: string) {
    toast = msg;
    setTimeout(() => (toast = null), 2600);
  }

  async function refresh() {
    try {
      snippets = await api.listSnippets();
      status = await api.getStatus();
      if (!importPath && status.fastkeys_db) importPath = status.fastkeys_db;
    } catch (e) {
      // window opened before backend ready
    }
  }

  function addNew() {
    editing = { id: "", trigger: "", replacement: "", enabled: true, case_sensitive: false, sensitive: false, trigger_mode: "auto", word_boundary: false, mode: null };
  }

  async function save() {
    if (!editing) return;
    if (!editing.trigger.trim()) {
      flash("Trigger is required");
      return;
    }
    snippets = await api.saveSnippet(editing);
    editing = null;
    flash("Saved");
  }

  async function remove(s: Snippet) {
    snippets = await api.deleteSnippet(s.id);
    if (editing?.id === s.id) editing = null;
    flash("Deleted " + s.trigger);
  }

  async function parseFile() {
    busy = true;
    try {
      report = await api.parseFdb(importPath);
      flash("Parsed " + report.parsed + " rules");
    } catch (e) {
      flash("Parse failed: " + String(e));
    }
    busy = false;
  }

  async function runImport() {
    busy = true;
    try {
      report = await api.importFdb(importPath, replaceExisting);
      snippets = await api.listSnippets();
      status = await api.getStatus();
      flash("Imported " + report.imported + " · skipped " + report.skipped_duplicates + " duplicate(s)");
    } catch (e) {
      flash("Import failed: " + String(e));
    }
    busy = false;
  }

  async function detectPath() {
    const p = await api.fastkeysPath();
    if (p) {
      importPath = p;
      flash("Found: " + p);
    } else {
      flash("No FastKeys data file detected");
    }
  }

  async function saveSettings() {
    if (!status) return;
    status = await api.updateSettings({
      default_mode: status.default_mode,
      paused: status.paused,
      autostart: status.autostart
    });
    flash("Settings saved");
  }

  async function toggleAutostart() {
    if (!status) return;
    try {
      status = await api.setAutostart(!status.autostart);
      flash(status.autostart ? "KeyFlow will start with Windows" : "Autostart disabled");
    } catch (e) {
      flash("Autostart failed: " + String(e));
    }
  }

  async function togglePaused() {
    if (!status) return;
    status = await api.setPaused(!status.paused);
  }

  async function sendTest() {
    if (!editing) return;
    flash("Focus your target window — typing in 3 seconds");
    await api.sendTest(editing.replacement, editing.mode ?? status?.default_mode ?? "fast", 3000);
  }

  onMount(() => {
    refresh();
    const un = listen("manager-tab", (e) => {
      const t = String(e.payload ?? "");
      if (t === "new") {
        tab = "snippets";
        addNew();
        tick().then(() => triggerInput?.focus());
      } else if (t === "snippets" || t === "import" || t === "settings" || t === "about") {
        tab = t as Tab;
      }
    });
    const un2 = listen("manager-open-snippet", (e) => {
      const p = e.payload as { mode: string; snippet: Snippet };
      tab = "snippets";
      if (p.mode === "copy") {
        editing = { ...p.snippet, id: "", trigger: "" };
        flash("Set a trigger for the copy");
      } else {
        editing = { ...(snippets.find((s) => s.id === p.snippet.id) ?? p.snippet) };
      }
      tick().then(() => triggerInput?.focus());
    });
    const un3 = listen("config-updated", () => {
      refresh();
    });
    return () => {
      un.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
    };
  });
</script>

<div class="min-h-screen bg-[#0d0f14] text-[13.5px] text-white/90" oncontextmenu={(e) => e.preventDefault()}>
  <header class="flex items-center gap-4 border-b border-white/8 px-5 py-3">
    <span class="text-[15px] font-semibold tracking-tight">KeyFlow</span>
    {#if status}<span class="text-[12px] text-white/30">v{status.version}</span>{/if}
    <nav class="ml-4 flex gap-1">
      {#each tabs as t (t.id)}
        <button
          type="button"
          onclick={() => (tab = t.id)}
          class={["rounded-lg px-3 py-1.5 text-[13px]", tab === t.id ? "bg-white/10 font-medium text-white" : "text-white/45 hover:text-white/80"].join(" ")}
        >{t.label}</button>
      {/each}
    </nav>
    <div class="ml-auto flex items-center gap-2">
      {#if status}
        <span class="text-[12px] text-white/35">{status.snippet_count} snippets</span>
        <button
          type="button"
          onclick={togglePaused}
          class={["rounded-lg px-3 py-1.5 text-[12.5px] font-medium", status.paused ? "bg-amber-400/15 text-amber-300" : "bg-emerald-400/10 text-emerald-300/90"].join(" ")}
        >{status.paused ? "Paused" : "Active"}</button>
      {/if}
    </div>
  </header>

  <main class="p-5">
    {#if tab === "snippets"}
      <div class="flex gap-5">
        <section class="flex w-[320px] shrink-0 flex-col">
          <div class="mb-3 flex gap-2">
            <input
              bind:value={filter}
              placeholder="Filter snippets…"
              spellcheck="false"
              class="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 outline-none focus:border-[#5B8CFF]/60"
            />
            <button type="button" onclick={addNew} class="shrink-0 rounded-lg bg-[#5B8CFF] px-3.5 py-2 font-medium text-white hover:bg-[#4a7cf5]">New</button>
          </div>
          <div class="max-h-[64vh] overflow-y-auto rounded-xl border border-white/8 bg-white/[0.02]">
            {#each shown as s (s.id)}
              <div class={["flex items-center gap-2 border-b border-white/5 px-3 py-2 last:border-b-0", editing?.id === s.id ? "bg-white/10" : "hover:bg-white/5"].join(" ")}>
                <button type="button" class="min-w-0 flex-1 text-left" onclick={() => (editing = { ...s })}>
                  <div class="truncate font-medium text-white/90">{s.trigger}</div>
                  <div class="flex min-w-0 items-center gap-1.5 text-[11.5px] text-white/35">
                    {#if s.sensitive}
                      <svg class="h-3 w-3 shrink-0 text-white/30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M7 11V8a5 5 0 0 1 10 0v3"></path><rect x="5" y="11" width="14" height="9" rx="2"></rect></svg>
                    {/if}
                    <div class={["min-w-0 truncate", s.sensitive ? "select-none blur-[5px] opacity-60" : ""].join(" ")}>{s.replacement.replace(/\s+/g, " ").slice(0, 44)}</div>
                  </div>
                </button>
                {#if !s.enabled}
                  <span class="shrink-0 rounded bg-white/5 px-1.5 py-0.5 text-[10px] text-white/40">off</span>
                {/if}
                {#if s.trigger_mode === "confirm"}
                  <span class="shrink-0 rounded bg-amber-400/15 px-1.5 py-0.5 text-[10px] font-medium text-amber-300">ASK</span>
                {:else if s.trigger_mode === "manual"}
                  <span class="shrink-0 rounded bg-white/10 px-1.5 py-0.5 text-[10px] font-medium text-white/50">MANUAL</span>
                {/if}
                <button type="button" onclick={() => remove(s)} class="shrink-0 px-1 text-white/25 hover:text-red-400" title="Delete">✕</button>
              </div>
            {/each}
            {#if shown.length === 0}
              <div class="px-3 py-12 text-center text-white/30">No snippets — press New or import from FastKeys</div>
            {/if}
          </div>
          <p class="mt-2 text-[11.5px] text-white/25">{snippets.length} snippets total</p>
        </section>

        <section class="min-w-0 flex-1">
          {#if editing}
            <div class="max-w-3xl space-y-4 rounded-xl border border-white/8 bg-white/[0.03] p-5" onkeydown={onEditorKey}>
              <div class="grid grid-cols-2 gap-4">
                <label class="block">
                  <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.12em] text-white/35">Trigger</span>
                  <input
                    bind:value={editing.trigger}
                    bind:this={triggerInput}
                    oninput={scheduleHistory}
                    spellcheck="false"
                    class="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono outline-none focus:border-[#5B8CFF]/60"
                  />
                </label>
                <label class="block">
                  <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.12em] text-white/35">Typing mode</span>
                  <select bind:value={editing.mode} class="w-full rounded-lg border border-white/10 bg-[#171a22] px-3 py-2 outline-none">
                    <option value={null}>default ({status?.default_mode ?? "fast"})</option>
                    {#each modes as m}<option value={m}>{m}</option>{/each}
                  </select>
                </label>
              </div>
              <div class="grid grid-cols-2 gap-4">
                <label class="block">
                  <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.12em] text-white/35">Trigger mode</span>
                  <select bind:value={editing.trigger_mode} class="w-full rounded-lg border border-white/10 bg-[#171a22] px-3 py-2 outline-none">
                    <option value="auto">Auto — expands while typing</option>
                    <option value="confirm">Confirm — press Enter to insert</option>
                    <option value="manual">Manual — only from Ctrl+Space</option>
                  </select>
                </label>
                <label class="flex items-end gap-2 pb-2.5 text-white/70" title="Only fire when the trigger starts at a word boundary (line start, space or punctuation)">
                  <input type="checkbox" bind:checked={editing.word_boundary} /> Standalone word only
                </label>
              </div>
              <label class="block">
                <span class="mb-1.5 block text-[11px] font-semibold uppercase tracking-[0.12em] text-white/35">
                  Replacement — supports {"{tab}"} {"{enter}"} {"{ctrl+x}"} {"{sleep 250}"}
                </span>
                <textarea
                  bind:this={replEl}
                  bind:value={editing.replacement}
                  oninput={scheduleHistory}
                  rows="6"
                  spellcheck="false"
                  ondragover={(e) => {
                    e.preventDefault();
                    dragOver = true;
                  }}
                  ondragleave={() => (dragOver = false)}
                  ondrop={(e) => {
                    e.preventDefault();
                    dragOver = false;
                    const data = e.dataTransfer?.getData("text/plain") ?? "";
                    if (data) insertAtCaret(data, tokenAtPoint(e.clientX, e.clientY));
                  }}
                  class={["w-full rounded-lg border bg-white/5 px-3 py-2 font-mono outline-none focus:border-[#5B8CFF]/60", dragOver ? "border-[#5B8CFF] ring-2 ring-[#5B8CFF]/30" : "border-white/10"].join(" ")}
                ></textarea>
              </label>
              {#if editing.replacement}
                <div class="flex flex-wrap items-center gap-1.5 text-[12px] leading-relaxed">
                  <span class="mr-1 text-[10.5px] font-semibold uppercase tracking-[0.14em] text-white/30">Preview</span>
                  {#each previewParts(editing.replacement) as part}
                    {#if part.isToken}
                      <span
                        class={["rounded px-1.5 py-0.5 font-mono text-[11px]", isValidToken(part.text) ? "bg-[#5B8CFF]/20 text-[#9DB6FF]" : "bg-amber-400/15 text-amber-300"].join(" ")}
                        title={isValidToken(part.text) ? "Executes as an action" : "Not a known action - typed literally"}
                      >{part.text}</span>
                    {:else}
                      <span class="whitespace-pre-wrap break-all text-white/50">{part.text}</span>
                    {/if}
                  {/each}
                </div>
              {/if}

              <div class="rounded-xl border border-white/8 bg-white/[0.02] p-3">
                <div class="mb-2 flex items-center justify-between">
                  <span class="text-[10.5px] font-semibold uppercase tracking-[0.14em] text-white/30">Insert action</span>
                  <span class="text-[11px] text-white/25">drag onto the field above, or click to insert at the caret</span>
                </div>

                <div class="mb-1.5 flex flex-wrap items-center gap-1.5">
                  <span class="w-14 shrink-0 text-[10.5px] font-semibold uppercase tracking-[0.12em] text-white/25">Keys</span>
                  {#each keyChips as chip (chip.token)}
                    <button
                      type="button"
                      draggable="true"
                      title={chip.token}
                      onclick={() => insertAtCaret(chip.token)}
                      ondragstart={(e) => {
                        e.dataTransfer?.setData("text/plain", chip.token);
                        if (e.dataTransfer) e.dataTransfer.effectAllowed = "copy";
                      }}
                      class="cursor-grab rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[11.5px] text-white/70 hover:bg-white/10 active:cursor-grabbing"
                    >{chip.label}</button>
                  {/each}
                  <select onchange={insertFKey} class="rounded-md border border-white/10 bg-[#171a22] px-1.5 py-1 text-[11.5px] text-white/60 outline-none">
                    <option value="">F-key…</option>
                    {#each fKeys as f (f)}<option value={f}>{f.toUpperCase()}</option>{/each}
                  </select>
                </div>

                <div class="mb-1.5 flex flex-wrap items-center gap-1.5">
                  <span class="w-14 shrink-0 text-[10.5px] font-semibold uppercase tracking-[0.12em] text-white/25">Combo</span>
                  {#each modDefs as md (md.key)}
                    <button
                      type="button"
                      onclick={() => toggleMod(md.key)}
                      class={["rounded-md border px-2 py-1 text-[11.5px]", comboMods[md.key] ? "border-[#5B8CFF]/60 bg-[#5B8CFF]/20 text-white" : "border-white/10 bg-white/5 text-white/50 hover:bg-white/10"].join(" ")}
                    >{md.label}</button>
                  {/each}
                  <select bind:value={comboKey} class="rounded-md border border-white/10 bg-[#171a22] px-1.5 py-1 text-[11.5px] text-white/60 outline-none">
                    <optgroup label="Letters">{#each letters as l (l)}<option value={l}>{l.toUpperCase()}</option>{/each}</optgroup>
                    <optgroup label="Digits">{#each digits as d (d)}<option value={d}>{d}</option>{/each}</optgroup>
                    <optgroup label="Named">{#each comboNamed as n (n)}<option value={n}>{n}</option>{/each}</optgroup>
                  </select>
                  <button
                    type="button"
                    draggable="true"
                    title={comboToken}
                    onclick={() => insertAtCaret(comboToken)}
                    ondragstart={(e) => {
                      e.dataTransfer?.setData("text/plain", comboToken);
                      if (e.dataTransfer) e.dataTransfer.effectAllowed = "copy";
                    }}
                    class="cursor-grab rounded-md border border-[#5B8CFF]/40 bg-[#5B8CFF]/15 px-2 py-1 font-mono text-[11.5px] text-[#9DB6FF] hover:bg-[#5B8CFF]/25 active:cursor-grabbing"
                  >{comboToken}</button>
                </div>

                <div class="mb-1.5 flex flex-wrap items-center gap-1.5">
                  <span class="w-14 shrink-0 text-[10.5px] font-semibold uppercase tracking-[0.12em] text-white/25">Mouse</span>
                  {#each mouseChips as chip (chip.token)}
                    <button
                      type="button"
                      draggable="true"
                      title={chip.token}
                      onclick={() => insertAtCaret(chip.token)}
                      ondragstart={(e) => {
                        e.dataTransfer?.setData("text/plain", chip.token);
                        if (e.dataTransfer) e.dataTransfer.effectAllowed = "copy";
                      }}
                      class="cursor-grab rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[11.5px] text-white/70 hover:bg-white/10 active:cursor-grabbing"
                    >{chip.label}</button>
                  {/each}
                </div>

                <div class="flex flex-wrap items-center gap-1.5">
                  <span class="w-14 shrink-0 text-[10.5px] font-semibold uppercase tracking-[0.12em] text-white/25">Timing</span>
                  <input type="number" bind:value={sleepMs} min="10" step="50" class="w-20 rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[11.5px] outline-none" />
                  <span class="text-[11px] text-white/35">ms</span>
                  <button
                    type="button"
                    draggable="true"
                    title={"sleep " + sleepMs}
                    onclick={() => insertAtCaret("{sleep " + sleepMs + "}")}
                    ondragstart={(e) => {
                      e.dataTransfer?.setData("text/plain", "{sleep " + sleepMs + "}");
                      if (e.dataTransfer) e.dataTransfer.effectAllowed = "copy";
                    }}
                    class="cursor-grab rounded-md border border-white/10 bg-white/5 px-2 py-1 font-mono text-[11.5px] text-white/70 hover:bg-white/10 active:cursor-grabbing"
                  >{"{sleep " + sleepMs + "}"}</button>
                </div>
              </div>
              <div class="flex items-center gap-6 text-white/70">
                <label class="flex items-center gap-2"><input type="checkbox" bind:checked={editing.enabled} /> Enabled</label>
                <label class="flex items-center gap-2"><input type="checkbox" bind:checked={editing.case_sensitive} /> Case sensitive</label>
                <label class="flex items-center gap-2" title="Blur this snippet's value in the palette and lists (for screenshots/recordings) until you edit it here">
                  <input type="checkbox" bind:checked={editing.sensitive} /> Sensitive
                </label>
              </div>
              <div class="flex gap-2 pt-1">
                <button type="button" onclick={save} class="rounded-lg bg-[#5B8CFF] px-4 py-2 font-medium text-white hover:bg-[#4a7cf5]">Save</button>
                <button type="button" onclick={() => (editing = null)} class="rounded-lg bg-white/5 px-4 py-2 text-white/70 hover:bg-white/10">Cancel</button>
                <button
                  type="button"
                  onclick={undoEdit}
                  disabled={!canUndo}
                  title="Undo (Ctrl+Z)"
                  class={["rounded-lg px-4 py-2", canUndo ? "bg-white/5 text-white/70 hover:bg-white/10" : "bg-white/5 text-white/25"].join(" ")}
                >Undo</button>
                <button
                  type="button"
                  onclick={redoEdit}
                  disabled={!canRedo}
                  title="Redo (Ctrl+Y)"
                  class={["rounded-lg px-4 py-2", canRedo ? "bg-white/5 text-white/70 hover:bg-white/10" : "bg-white/5 text-white/25"].join(" ")}
                >Redo</button>
                <button type="button" onclick={() => api.copyText(editing?.replacement ?? "")} class="rounded-lg bg-white/5 px-4 py-2 text-white/70 hover:bg-white/10">Copy</button>
                <button type="button" onclick={sendTest} class="rounded-lg bg-white/5 px-4 py-2 text-white/70 hover:bg-white/10">Send test (3s)</button>
              </div>
            </div>
          {:else}
            <div class="flex h-64 max-w-3xl items-center justify-center rounded-xl border border-dashed border-white/10 text-white/30">
              Select a snippet to edit, or press New
            </div>
          {/if}
        </section>
      </div>
    {:else if tab === "import"}
      <div class="max-w-4xl">
        <div class="rounded-xl border border-white/8 bg-white/[0.03] p-5">
          <h2 class="mb-2 text-[14.5px] font-semibold">Import from FastKeys</h2>
          <p class="mb-4 text-[13px] leading-relaxed text-white/50">
            Reads your FastKeys phrase database (<span class="font-mono text-white/70">.fdb</span>) and converts its Send-style
            entries into KeyFlow snippets. The original file is never modified.
          </p>
          <div class="mb-4 flex gap-2">
            <input
              bind:value={importPath}
              spellcheck="false"
              placeholder="Path to FASTKEYS.fdb"
              class="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[12.5px] outline-none focus:border-[#5B8CFF]/60"
            />
            <button type="button" onclick={detectPath} class="shrink-0 rounded-lg bg-white/5 px-3.5 py-2 text-white/70 hover:bg-white/10">Detect</button>
            <button type="button" onclick={parseFile} disabled={busy} class="shrink-0 rounded-lg bg-white/10 px-3.5 py-2 text-white/85 hover:bg-white/15">Parse</button>
          </div>
          {#if report}
            <div class="mb-4 flex flex-wrap items-center gap-4">
              <span class="text-white/60">{report.parsed} rules parsed from {report.total_lines} lines</span>
              <label class="flex items-center gap-2 text-white/60"><input type="checkbox" bind:checked={replaceExisting} /> Replace existing triggers</label>
              <button type="button" onclick={runImport} disabled={busy} class="rounded-lg bg-[#5B8CFF] px-4 py-2 font-medium text-white hover:bg-[#4a7cf5]">
                Import {report.parsed} rules
              </button>
            </div>
            <div class="max-h-[48vh] overflow-y-auto rounded-lg border border-white/8">
              <table class="w-full text-left">
                <thead class="sticky top-0 bg-[#14161e] text-[11px] uppercase tracking-wide text-white/35">
                  <tr>
                    <th class="px-3 py-2 font-semibold">Trigger</th>
                    <th class="px-3 py-2 font-semibold">Name</th>
                    <th class="px-3 py-2 font-semibold">Replacement preview</th>
                  </tr>
                </thead>
                <tbody>
                  {#each report.rules as r, i (r.trigger + "#" + i)}
                    <tr class="border-t border-white/5">
                      <td class="px-3 py-1.5 font-mono text-[12.5px]">{r.trigger}</td>
                      <td class="px-3 py-1.5 text-white/45">{r.name}</td>
                      <td class="px-3 py-1.5 font-mono text-[12px] text-white/40">{r.preview}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </div>
      </div>
    {:else if tab === "settings"}
      <div class="max-w-2xl">
        {#if status}
          <div class="rounded-xl border border-white/8 bg-white/[0.03] p-5">
            <h2 class="mb-2 text-[14.5px] font-semibold">General</h2>
            <div class="divide-y divide-white/5">
              <label class="flex items-center justify-between gap-6 py-3.5">
                <span class="text-white/75">Default typing mode</span>
                  <select bind:value={status.default_mode} onchange={saveSettings} class="w-44 rounded-lg border border-white/10 bg-[#171a22] px-3 py-2 outline-none">
                  {#each modes as m}<option value={m}>{m}</option>{/each}
                </select>
              </label>
              <div class="flex items-center justify-between gap-6 py-3.5">
                <span class="text-white/75">Start with Windows</span>
                <button
                  type="button"
                  onclick={toggleAutostart}
                  class={["rounded-lg px-3.5 py-1.5 text-[12.5px] font-medium", status.autostart ? "bg-emerald-400/15 text-emerald-300" : "bg-white/5 text-white/55 hover:bg-white/10"].join(" ")}
                >{status.autostart ? "Enabled" : "Disabled"}</button>
              </div>
              <div class="flex items-center justify-between gap-6 py-3.5">
                <span class="text-white/75">Expansion paused</span>
                <button
                  type="button"
                  onclick={togglePaused}
                  class={["rounded-lg px-3.5 py-1.5 text-[12.5px] font-medium", status.paused ? "bg-amber-400/15 text-amber-300" : "bg-white/5 text-white/55 hover:bg-white/10"].join(" ")}
                >{status.paused ? "Paused" : "Running"}</button>
              </div>
              <div class="flex items-center justify-between gap-6 py-3.5">
                <span class="text-white/75">Palette hotkey</span>
                <span class="flex items-center gap-1.5">
                  <kbd class="rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[12px] text-white/70">Ctrl</kbd>
                  <span class="text-white/30">+</span>
                  <kbd class="rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[12px] text-white/70">Space</kbd>
                </span>
              </div>
            </div>
            <div class="flex gap-2 pt-4">
              <button type="button" onclick={saveSettings} class="rounded-lg bg-[#5B8CFF] px-4 py-2 font-medium text-white hover:bg-[#4a7cf5]">Save</button>
              <button
                type="button"
                onclick={async () => {
                  status = await api.reloadConfig();
                  await refresh();
                  flash("Configuration reloaded");
                }}
                class="rounded-lg bg-white/5 px-4 py-2 text-white/70 hover:bg-white/10"
              >Reload from disk</button>
            </div>
          </div>
        {/if}
      </div>
    {:else}
      <div class="max-w-2xl space-y-4 text-[13px] leading-relaxed text-white/55">
        <div class="rounded-xl border border-white/8 bg-white/[0.03] p-5">
          <h2 class="mb-2 text-[14.5px] font-semibold text-white/90">About KeyFlow</h2>
          <p class="mb-3">
            A FastKeys-style text expander for Windows, built with the same stack as asyar: Tauri 2, Rust and Svelte 5.
            Type a trigger anywhere and KeyFlow replaces it — including {"{tab}"}, {"{enter}"} and other keys.
          </p>
          <p class="mb-3">
            Configuration lives in <span class="font-mono text-white/75">%APPDATA%\KeyFlow\config.json</span>. Typing modes:
            <span class="text-white/75">instant</span> (single batch; longer text is pasted via the clipboard for speed),
            <span class="text-white/75">fast</span>, <span class="text-white/75">human</span> (jittered per-character delays),
            <span class="text-white/75">slow</span>.
          </p>
          <p>
            Notes: Windows blocks input into elevated (admin) windows from normal apps, and anti-cheat games may ignore hook
            input. Quit FastKeys while KeyFlow runs to avoid double expansion of the same triggers.
          </p>
        </div>
      </div>
    {/if}
  </main>

  {#if toast}
    <div class="pointer-events-none fixed bottom-5 left-1/2 -translate-x-1/2 rounded-lg border border-white/10 bg-[#1a1d26] px-4 py-2 text-[12.5px] text-white/80 shadow-xl">
      {toast}
    </div>
  {/if}
</div>
