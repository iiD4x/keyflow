import { invoke } from "@tauri-apps/api/core";

export type TypingMode = "instant" | "fast" | "human" | "slow";
export type TriggerMode = "auto" | "confirm" | "manual";

export interface Snippet {
  id: string;
  trigger: string;
  replacement: string;
  enabled: boolean;
  case_sensitive: boolean;
  sensitive: boolean;
  trigger_mode: TriggerMode;
  word_boundary: boolean;
  mode?: TypingMode | null;
}

export interface Settings {
  default_mode: TypingMode;
  paused: boolean;
  autostart: boolean;
}

export interface Status {
  paused: boolean;
  autostart: boolean;
  default_mode: TypingMode;
  snippet_count: number;
  version: string;
  fastkeys_db: string | null;
}

export interface ImportRule {
  trigger: string;
  name: string;
  preview: string;
}

export interface ImportReport {
  total_lines: number;
  parsed: number;
  imported: number;
  skipped_duplicates: number;
  rules: ImportRule[];
}

export const api = {
  listSnippets: () => invoke<Snippet[]>("list_snippets"),
  saveSnippet: (snippet: Snippet) => invoke<Snippet[]>("save_snippet", { snippet }),
  deleteSnippet: (id: string) => invoke<Snippet[]>("delete_snippet", { id }),
  getStatus: () => invoke<Status>("get_status"),
  updateSettings: (settings: Settings) => invoke<Status>("update_settings", { settings }),
  setPaused: (paused: boolean) => invoke<Status>("set_paused", { paused }),
  setAutostart: (autostart: boolean) => invoke<Status>("set_autostart", { autostart }),
  reloadConfig: () => invoke<Status>("reload_config"),
  executeSnippet: (id: string) => invoke<void>("execute_snippet", { id }),
  copySnippet: (id: string) => invoke<void>("copy_snippet", { id }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  hidePalette: () => invoke<void>("hide_palette"),
  openManager: (tab: string | null) => invoke<void>("open_manager", { tab }),
  openManagerSnippet: (mode: "edit" | "copy", snippet: Snippet) => invoke<void>("open_manager_snippet", { mode, snippet }),
  sendTest: (text: string, mode: TypingMode, delayMs: number) => invoke<void>("send_test", { text, mode, delayMs }),
  fastkeysPath: () => invoke<string | null>("fastkeys_db_path"),
  parseFdb: (path: string) => invoke<ImportReport>("parse_fdb", { path }),
  importFdb: (path: string, replace: boolean) => invoke<ImportReport>("import_fdb", { path, replace })
};
