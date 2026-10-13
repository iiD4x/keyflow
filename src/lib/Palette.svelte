<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, type Snippet, type Status, type TriggerMode, type TypingMode, type Widget } from "$lib/api";

  type SnippetItem = { kind: "snippet"; id: string; title: string; sub: string; enabled: boolean; sensitive: boolean; triggerMode: TriggerMode; mode?: string | null };
  type Item = SnippetItem | Tile;

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
  let tick = $state(0);
  let weather = $state<Record<string, { temp: number | null; code: number | null; error: string; at: number }>>({});
  let tileMenu = $state<{ x: number; y: number; id: string } | null>(null);
  let addMenu = $state<{ x: number; y: number } | null>(null);
  let cfgPanel = $state<{ id: string; kind: string; city: string; tz: string } | null>(null);
  let cfgEl = $state<HTMLInputElement>();

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

  let gridEl = $state<HTMLDivElement>();

  function gridCols(): number {
    const el = gridEl;
    if (!el || el.children.length === 0) return 5;
    const top = (el.children[0] as HTMLElement).offsetTop;
    let c = 0;
    for (const child of Array.from(el.children)) {
      if ((child as HTMLElement).offsetTop === top) c += 1;
      else break;
    }
    return Math.max(1, c);
  }

  function moveVert(delta: number): void {
    const n = results.length;
    if (n === 0) return;
    const tc = tiles.length;
    if (sel < tc) {
      const next = sel + delta * gridCols();
      if (next >= 0 && next < tc) {
        sel = next;
        return;
      }
      if (delta < 0) return;
      sel = tc < n ? tc : sel;
      return;
    }
    const next = sel + delta;
    if (next < tc) {
      sel = Math.max(0, Math.min(tc - 1, next));
      return;
    }
    sel = Math.max(0, Math.min(n - 1, next));
  }

  function moveSide(delta: number): void {
    const tc = tiles.length;
    if (sel >= tc) return;
    const next = sel + delta;
    if (next < 0 || next >= tc) return;
    if (Math.floor(next / gridCols()) !== Math.floor(sel / gridCols())) return;
    sel = next;
  }

  type Tile = {
    id: string;
    kind: "tile";
    tkind: "action" | "widget" | "add";
    action?: string;
    wkind?: string;
    icon: string;
    label: string;
    value: string;
    sub: string;
    configurable: boolean;
  };

  const ICONS: Record<string, string> = {
    plus: '<path d="M12 5v14M5 12h14"></path>',
    gear: '<circle cx="12" cy="12" r="3"></circle><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1"></path>',
    pause: '<path d="M9 5v14M15 5v14"></path>',
    play: '<path d="M8 5l11 7-11 7z"></path>',
    import: '<path d="M12 3v12M7 10l5 5 5-5M5 21h14"></path>',
    reload: '<path d="M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5"></path>',
    clock: '<circle cx="12" cy="12" r="9"></circle><path d="M12 7v5l3 2"></path>',
    date: '<rect x="3" y="5" width="18" height="16" rx="2"></rect><path d="M8 3v4M16 3v4M3 10h18"></path>',
    weather: '<path d="M7 18h9a4 4 0 0 0 0-8 5.5 5.5 0 0 0-10.5 1.5A3.5 3.5 0 0 0 7 18z"></path>',
    globe: '<circle cx="12" cy="12" r="9"></circle><path d="M3 12h18M12 3c2.5 3 2.5 15 0 18M12 3c-2.5 3-2.5 15 0 18"></path>'
  };

  const WMO: Record<number, string> = {
    0: "Clear", 1: "Mostly clear", 2: "Partly cloudy", 3: "Cloudy",
    45: "Fog", 48: "Rime fog", 51: "Light drizzle", 53: "Drizzle", 55: "Drizzle",
    61: "Light rain", 63: "Rain", 65: "Heavy rain", 66: "Freezing rain", 67: "Freezing rain",
    71: "Light snow", 73: "Snow", 75: "Heavy snow", 77: "Snow grains",
    80: "Showers", 81: "Showers", 82: "Heavy showers",
    85: "Snow showers", 86: "Snow showers", 95: "Thunderstorm", 96: "Thunderstorm", 99: "Thunderstorm"
  };

  const ADD_TYPES: { kind: string; label: string; hint: string; icon: string }[] = [
    { kind: "clock", label: "Clock", hint: "Live local time", icon: "clock" },
    { kind: "date", label: "Date", hint: "Today, day and month", icon: "date" },
    { kind: "weather", label: "Weather", hint: "Current conditions for a city", icon: "weather" },
    { kind: "worldclock", label: "World clock", hint: "Another time zone", icon: "globe" },
    { kind: "import", label: "Import FastKeys rules", hint: "Opens the Backup tab", icon: "import" },
    { kind: "reload", label: "Reload configuration", hint: "Re-read config.json from disk", icon: "reload" }
  ];

  const QUICK_TZ = ["UTC", "Asia/Riyadh", "Europe/London", "America/New_York", "Asia/Dubai", "Asia/Tokyo"];

  const liveNow = $derived.by(() => {
    void tick;
    return new Date();
  });

  const widgets = $derived(status?.widgets ?? []);

  function seg(path: string): string {
    const parts = path.split("/");
    return (parts[parts.length - 1] ?? path).replace(/_/g, " ");
  }

  const fmt = (opts: Intl.DateTimeFormatOptions, tz?: string) => {
    try {
      return new Intl.DateTimeFormat(undefined, tz ? { ...opts, timeZone: tz } : opts).format(liveNow);
    } catch {
      return "--";
    }
  };

  const tiles = $derived.by((): Tile[] => {
    if (!status) return [];
    const out: Tile[] = [
      { id: "new", kind: "tile", tkind: "action", action: "new", icon: "plus", label: "New snippet", value: "", sub: "", configurable: false },
      { id: "settings", kind: "tile", tkind: "action", action: "settings", icon: "gear", label: "Settings", value: "", sub: "", configurable: false },
      {
        id: "pause",
        kind: "tile",
        tkind: "action",
        action: "pause",
        icon: status.paused ? "play" : "pause",
        label: status.paused ? "Resume" : "Pause",
        value: "",
        sub: "",
        configurable: false
      }
    ];

    for (const w of status.widgets ?? []) {
      if (w.kind === "clock") {
        out.push({ id: w.id, kind: "tile", tkind: "widget", wkind: "clock", icon: "clock", label: "Clock", value: fmt({ hour: "2-digit", minute: "2-digit", hour12: false }), sub: fmt({ second: "2-digit" }) + "s", configurable: false });
      } else if (w.kind === "date") {
        out.push({ id: w.id, kind: "tile", tkind: "widget", wkind: "date", icon: "date", label: "Date", value: fmt({ day: "numeric", month: "short" }), sub: fmt({ weekday: "long" }), configurable: false });
      } else if (w.kind === "weather") {
        const st = weather[w.id];
        const temp = st && typeof st.temp === "number" ? Math.round(st.temp) + "°" : "—";
        const sub = !st ? "…" : st.error && st.error !== "loading" ? st.error : st.code != null ? WMO[st.code] ?? "—" : "loading…";
        out.push({ id: w.id, kind: "tile", tkind: "widget", wkind: "weather", icon: "weather", label: (w.city || "City").trim(), value: temp, sub, configurable: true });
      } else if (w.kind === "worldclock") {
        out.push({ id: w.id, kind: "tile", tkind: "widget", wkind: "worldclock", icon: "globe", label: seg(w.tz || "UTC"), value: fmt({ hour: "2-digit", minute: "2-digit", hour12: false }, w.tz || "UTC"), sub: fmt({ weekday: "short" }, w.tz || "UTC"), configurable: true });
      } else if (w.kind === "import") {
        out.push({ id: w.id, kind: "tile", tkind: "widget", wkind: "import", icon: "import", label: "Import", value: "", sub: "FastKeys", configurable: false });
      } else if (w.kind === "reload") {
        out.push({ id: w.id, kind: "tile", tkind: "widget", wkind: "reload", icon: "reload", label: "Reload", value: "", sub: "config", configurable: false });
      }
    }

    const q = query.trim().toLowerCase();
    const shown = !q ? out : out.filter((t) => (t.label + " " + t.sub + " " + t.value).toLowerCase().includes(q));
    return [...shown, { id: "add", kind: "tile", tkind: "add", icon: "plus", label: "Add", value: "", sub: "widget", configurable: false }];
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

  const results = $derived([...tiles, ...snips]);

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
    if (item.kind === "tile") {
      if (item.tkind === "add") {
        addMenu = { x: 24, y: 96 };
        return;
      }
      if (item.tkind === "widget") {
        if (item.wkind === "import") {
          await api.openManager("backup");
          await api.hidePalette();
          return;
        }
        if (item.wkind === "reload") {
          status = await api.reloadConfig();
          snippets = await api.listSnippets();
          return;
        }
        const w = widgets.find((x) => x.id === item.id);
        if (w && item.configurable) openCfg(w);
        return;
      }
      if (item.action === "new") await api.openManager("new");
      else if (item.action === "pause") {
        status = await api.setPaused(!(status?.paused ?? false));
        sel = 0;
      } else if (item.action === "settings") await api.openManager(null);
      return;
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

  async function saveWidgets(list: Widget[]): Promise<void> {
    try {
      status = await api.saveWidgets(list);
    } catch {
      // the palette stays usable even if the write fails
    }
  }

  function addWidget(kind: string): void {
    const w: Widget = { id: "w" + Date.now().toString(36) + Math.random().toString(36).slice(2, 5), kind };
    if (kind === "weather") w.city = "Riyadh";
    if (kind === "worldclock") w.tz = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
    addMenu = null;
    void saveWidgets([...widgets, w]);
    if (kind === "weather" || kind === "worldclock") openCfg(w);
  }

  function openCfg(w: Widget): void {
    cfgPanel = { id: w.id, kind: w.kind, city: w.city ?? "", tz: w.tz ?? "" };
    tileMenu = null;
    addMenu = null;
    setTimeout(() => cfgEl?.focus(), 40);
  }

  function removeWidget(id: string): void {
    tileMenu = null;
    void saveWidgets(widgets.filter((w) => w.id !== id));
  }

  function moveWidget(id: string, dir: number): void {
    const i = widgets.findIndex((w) => w.id === id);
    const j = i + dir;
    tileMenu = null;
    if (i < 0 || j < 0 || j >= widgets.length) return;
    const list = widgets.slice();
    const tmp = list[i];
    list[i] = list[j];
    list[j] = tmp;
    void saveWidgets(list);
  }

  function commitCfg(): void {
    const c = cfgPanel;
    if (!c) return;
    cfgPanel = null;
    void saveWidgets(widgets.map((w) => (w.id === c.id ? { ...w, city: c.city.trim(), tz: c.tz.trim() || w.tz } : w)));
  }

  async function fetchWeather(): Promise<void> {
    for (const w of widgets) {
      if (w.kind !== "weather") continue;
      const city = (w.city ?? "").trim();
      if (!city) continue;
      const cached = weather[w.id];
      if (cached && cached.error === "" && Date.now() - cached.at < 15 * 60 * 1000) continue;
      if (cached && cached.error === "loading" && Date.now() - cached.at < 30000) continue;
      weather = { ...weather, [w.id]: { temp: null, code: null, error: "loading", at: Date.now() } };
      try {
        const geo = await fetch("https://geocoding-api.open-meteo.com/v1/search?count=1&name=" + encodeURIComponent(city)).then((r) => r.json());
        const hit = geo?.results?.[0];
        if (!hit) {
          weather = { ...weather, [w.id]: { temp: null, code: null, error: "city not found", at: Date.now() } };
          continue;
        }
        const fc = await fetch(
          "https://api.open-meteo.com/v1/forecast?current=temperature_2m,weather_code&latitude=" + hit.latitude + "&longitude=" + hit.longitude
        ).then((r) => r.json());
        const cur = fc?.current;
        weather = {
          ...weather,
          [w.id]: {
            temp: typeof cur?.temperature_2m === "number" ? cur.temperature_2m : null,
            code: typeof cur?.weather_code === "number" ? cur.weather_code : null,
            error: cur ? "" : "no data",
            at: Date.now()
          }
        };
      } catch {
        weather = { ...weather, [w.id]: { temp: null, code: null, error: "offline", at: Date.now() } };
      }
    }
  }

  function openTileMenu(e: MouseEvent, id: string): void {
    e.preventDefault();
    e.stopPropagation();
    closeMenu();
    tileMenu = { x: Math.max(8, Math.min(e.clientX, window.innerWidth - 240)), y: Math.min(e.clientY, window.innerHeight - 210), id };
  }

  function openAddMenu(e: MouseEvent): void {
    e.preventDefault();
    e.stopPropagation();
    closeMenu();
    addMenu = { x: Math.max(8, Math.min(e.clientX, window.innerWidth - 270)), y: Math.min(e.clientY, window.innerHeight - 330) };
  }

  function closeOverlays(): void {
    closeMenu();
    tileMenu = null;
    addMenu = null;
    cfgPanel = null;
  }

  $effect(() => {
    const sig = widgets.map((w) => w.id + ":" + w.kind + ":" + (w.city ?? "") + ":" + (w.tz ?? "")).join("|");
    if (!sig) return;
    void fetchWeather();
  });

  function onKey(e: KeyboardEvent) {
    if (cfgPanel) {
      if (e.key === "Escape") {
        e.preventDefault();
        cfgPanel = null;
      }
      return;
    }
    if (addMenu || tileMenu) {
      if (e.key === "Escape") {
        e.preventDefault();
        addMenu = null;
        tileMenu = null;
      }
      return;
    }

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
      moveVert(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      moveVert(-1);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      moveSide(-1);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      moveSide(1);
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
    {#if tiles.length > 0}
      <div class="flex items-baseline justify-between px-3 pb-2 pt-2">
        <span class="text-[11px] font-semibold uppercase tracking-[0.16em] text-white/30">Dashboard</span>
        <span class="text-[10.5px] text-white/25">right-click a tile to edit</span>
      </div>
      <div bind:this={gridEl} class="grid grid-cols-[repeat(auto-fill,minmax(104px,1fr))] gap-2 px-3 pb-3">
        {#each tiles as item, i (item.kind + item.id)}
          <button
            type="button"
            onmouseenter={() => (sel = i)}
            onclick={(e) => (item.tkind === "add" ? openAddMenu(e) : activate(item))}
            oncontextmenu={(e) => (item.tkind === "add" ? openAddMenu(e) : item.tkind === "widget" ? openTileMenu(e, item.id) : e.preventDefault())}
            class={["group flex aspect-square flex-col items-center justify-center gap-1 rounded-2xl border px-1.5 text-center transition", i === sel ? "border-[#5B8CFF]/60 bg-[#5B8CFF]/12" : "border-white/8 bg-white/[0.04] hover:border-white/15 hover:bg-white/[0.07]"].join(" ")}
          >
            {#if item.tkind === "add"}
              <span class="flex h-7 w-7 items-center justify-center rounded-xl border border-dashed border-white/25 text-white/40 group-hover:border-white/45 group-hover:text-white/75">
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">{@html ICONS.plus}</svg>
              </span>
              <span class="text-[10.5px] font-medium uppercase tracking-wide text-white/40">Add</span>
            {:else if item.tkind === "action"}
              <svg class="h-[26px] w-[26px] text-[#9DB6FF]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">{@html ICONS[item.icon]}</svg>
              <span class="max-w-full truncate text-[11px] font-medium leading-tight text-white/75">{item.label}</span>
            {:else}
              <svg class="h-[15px] w-[15px] text-white/30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">{@html ICONS[item.icon]}</svg>
              {#if item.value}
                <span class="text-[19px] font-semibold leading-none text-white/95">{item.value}</span>
              {/if}
              <span class="max-w-full truncate text-[10.5px] font-medium uppercase tracking-wide text-white/40">{item.label}</span>
              {#if item.sub}
                <span class="max-w-full truncate text-[10px] leading-none text-white/30">{item.sub}</span>
              {/if}
            {/if}
          </button>
        {/each}
      </div>
    {/if}

    {#if snips.length > 0}
      <div class="px-3 pb-1 pt-3 text-[11px] font-semibold uppercase tracking-[0.16em] text-white/30">Snippets</div>
      {#each snips as item, i (item.kind + item.id)}
        <button
          type="button"
          data-idx={tiles.length + i}
          onmouseenter={() => (sel = tiles.length + i)}
          onclick={() => activate(item)}
          oncontextmenu={(e) => openMenuAtCursor(e, item.id)}
          class={["flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left", tiles.length + i === sel ? "bg-white/8" : "hover:bg-white/5"].join(" ")}
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

  {#if addMenu}
    <div class="fixed inset-0 z-40" onclick={() => (addMenu = null)} oncontextmenu={(e) => { e.preventDefault(); addMenu = null; }}></div>
    <div
      class="absolute z-50 w-[262px] overflow-hidden rounded-xl border border-white/10 bg-[rgba(20,22,30,0.97)] py-1.5 shadow-[0_18px_48px_rgba(0,0,0,0.65)]"
      style={"left: " + addMenu.x + "px; top: " + addMenu.y + "px"}
      oncontextmenu={(e) => e.preventDefault()}
    >
      <div class="px-3 pb-1 pt-1 text-[10.5px] font-semibold uppercase tracking-[0.14em] text-white/30">Add widget</div>
      {#each ADD_TYPES as t (t.kind)}
        <button type="button" onclick={() => addWidget(t.kind)} class="flex w-full items-center gap-2.5 px-3 py-[7px] text-left hover:bg-white/5">
          <svg class="h-4 w-4 shrink-0 text-white/45" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">{@html ICONS[t.icon]}</svg>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[13px] text-white/85">{t.label}</span>
            <span class="block truncate text-[11px] text-white/35">{t.hint}</span>
          </span>
        </button>
      {/each}
    </div>
  {/if}

  {#if tileMenu}
    {@const tid = tileMenu.id}
    {@const tw = widgets.find((w) => w.id === tid)}
    {#if tw}
      <div class="fixed inset-0 z-40" onclick={() => (tileMenu = null)} oncontextmenu={(e) => { e.preventDefault(); tileMenu = null; }}></div>
      <div
        class="absolute z-50 w-[228px] overflow-hidden rounded-xl border border-white/10 bg-[rgba(20,22,30,0.97)] py-1.5 shadow-[0_18px_48px_rgba(0,0,0,0.65)]"
        style={"left: " + tileMenu.x + "px; top: " + tileMenu.y + "px"}
        oncontextmenu={(e) => e.preventDefault()}
      >
        {#if tw.kind === "weather" || tw.kind === "worldclock"}
          <button type="button" onclick={() => openCfg(tw)} class="flex w-full items-center justify-between gap-3 px-3 py-[6px] text-left text-[13px] text-white/85 hover:bg-white/5">
            <span>Configure…</span>
            <span class="max-w-[110px] truncate text-[11px] text-white/30">{tw.kind === "weather" ? tw.city : tw.tz}</span>
          </button>
          <div class="my-[3px] border-t border-white/5"></div>
        {/if}
        <button type="button" onclick={() => moveWidget(tw.id, -1)} class="flex w-full items-center px-3 py-[6px] text-left text-[13px] text-white/85 hover:bg-white/5">Move left</button>
        <button type="button" onclick={() => moveWidget(tw.id, 1)} class="flex w-full items-center px-3 py-[6px] text-left text-[13px] text-white/85 hover:bg-white/5">Move right</button>
        <div class="my-[3px] border-t border-white/5"></div>
        <button type="button" onclick={() => removeWidget(tw.id)} class="flex w-full items-center px-3 py-[6px] text-left text-[13px] text-red-400 hover:bg-red-500/10">Remove</button>
      </div>
    {/if}
  {/if}

  {#if cfgPanel}
    <div class="fixed inset-0 z-40 bg-black/45" onclick={() => (cfgPanel = null)}></div>
    <div
      class="absolute left-1/2 top-1/2 z-50 w-[344px] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-white/10 bg-[rgba(20,22,30,0.98)] p-4 shadow-[0_24px_64px_rgba(0,0,0,0.7)]"
      oncontextmenu={(e) => e.preventDefault()}
    >
      <div class="mb-3 text-[13.5px] font-semibold text-white/90">{cfgPanel.kind === "weather" ? "Weather location" : "Time zone"}</div>
      {#if cfgPanel.kind === "weather"}
        <input
          bind:this={cfgEl}
          bind:value={cfgPanel.city}
          placeholder="City, e.g. Riyadh"
          spellcheck="false"
          onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); commitCfg(); } }}
          class="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-[13px] text-white/90 outline-none focus:border-[#5B8CFF]/60"
        />
        <p class="mt-2 text-[11.5px] leading-relaxed text-white/35">Weather comes from Open-Meteo. Only weather tiles make a network request.</p>
      {:else}
        <input
          bind:this={cfgEl}
          bind:value={cfgPanel.tz}
          placeholder="Area/City, e.g. Asia/Tokyo"
          spellcheck="false"
          onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); commitCfg(); } }}
          class="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[12.5px] text-white/90 outline-none focus:border-[#5B8CFF]/60"
        />
        <div class="mt-2 flex flex-wrap gap-1.5">
          {#each QUICK_TZ as z (z)}
            <button type="button" onclick={() => cfgPanel && (cfgPanel.tz = z)} class="rounded-md border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-white/55 hover:bg-white/10">{z}</button>
          {/each}
        </div>
      {/if}
      <div class="mt-4 flex justify-end gap-2">
        <button type="button" onclick={() => (cfgPanel = null)} class="rounded-lg bg-white/5 px-3 py-1.5 text-[12.5px] text-white/60 hover:bg-white/10">Cancel</button>
        <button type="button" onclick={commitCfg} class="rounded-lg bg-[#5B8CFF] px-3.5 py-1.5 text-[12.5px] font-medium text-white hover:bg-[#4a7cf5]">Save</button>
      </div>
    </div>
  {/if}

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
