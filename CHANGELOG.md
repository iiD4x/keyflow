# Changelog

## 0.2.4

- Data safety: the previous configuration is snapshotted to config.backup.json before every save, and config.json is written through a temporary file followed by an atomic rename, so an interrupted or failed write can no longer truncate your snippets
- Data safety: if config.json cannot be parsed it is preserved as config.unreadable.json instead of being overwritten, and the reason is written to the log
- The configuration watcher logs unreadable or invalid JSON instead of silently continuing
- KEYFLOW_CONFIG_DIR environment variable overrides the data directory, so development, demo and screenshot runs cannot touch your real configuration

## 0.2.3

- Fixed: the confirmation prompt could appear with an empty trigger, because the confirm window was missing from the capability file and had no permission to receive events or call commands
- Fixed: the Settings window did not refresh when the configuration changed on disk
- Documentation: screenshots and an animated GIF of the palette with demo data

## 0.2.2

- Repository metadata for public release: neutral application identifier (com.keyflow.app), MIT license, contributor author field
- Reverse-engineering and format notes rewritten without machine-specific paths or personal examples

## 0.2.1

- Fixed: the space key was consumed by the Ctrl+Space hotkey check and never reached the matcher buffer, so a trigger could still match when a space had been typed between its letters

## 0.2.0

- Per-snippet trigger modes: Auto, Confirm (an always-on-top prompt waits for Enter, Esc cancels, any other key keeps typing) and Manual (reachable only from the palette)
- "Standalone word only" option per snippet
- Matcher hygiene: the buffer resets on Esc, after about 2.5s idle, on mouse clicks and on window switches
- Added a mouse hook and the confirmation overlay window

## 0.1.7

- Suppressed the WebView2 default context menu in both windows; snippet rows keep the custom options menu

## 0.1.6

- Per-snippet Sensitive flag: the value is blurred with a lock icon in the palette and the settings list, and visible only in the editor

## 0.1.5

- Undo and redo in the snippet editor (Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y, plus buttons), covering typing and inserted actions

## 0.1.4

- Editor "Insert action" palette (keys, combo builder, mouse, timing) with drag and drop onto the replacement field
- Live preview that highlights valid tokens; DPI-aware screenshot tooling for tests

## 0.1.3

- Token parser accepts letters and digits as the final key of a modifier combo ({ctrl+x}, {alt+f4})

## 0.1.2

- Snippet context menu in the palette, keyboard access with Shift+F10, taller palette window

## 0.1.1

- Instant typing mode inserts text with a single clipboard paste (with automatic fallback), bypassing slow per-keystroke injection
- Palette quick actions (New snippet, Settings, Import), Ctrl+N, configuration hot reload

## 0.1.0

- First build: global text expansion, token support, typing modes, Ctrl+Space palette, manager window, tray, FastKeys .fdb importer, NSIS installer
