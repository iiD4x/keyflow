# Contributing

Issues and pull requests are welcome.

## Reporting a bug

Use the bug report template. Include the KeyFlow version (Settings > About, or the installer file name), your Windows build, the exact keystrokes or clicks you made, and what you expected to happen. The log at %APPDATA%\KeyFlow\keyflow.log records each expansion with its mode, character counts and duration - redact anything private before pasting excerpts.

## Local setup

Prerequisites: Node 22+, Rust stable with the MSVC toolchain, and Visual Studio Build Tools with the C++ workload and the Windows SDK.

    npm install
    npm run tauri dev      # run with hot reload
    npm run tauri build    # produce the NSIS installer

The installer lands in src-tauri/target/release/bundle/nsis/.

## How the app is put together

- src-tauri/src/win.rs - raw Win32 FFI declarations (no external windows crate)
- src-tauri/src/engine.rs - global keyboard and mouse hooks, the keystroke buffer, trigger matching, trigger modes, confirmation handshake
- src-tauri/src/exec.rs - SendInput executor, token parser, typing modes
- src-tauri/src/config.rs - config model and persistence (%APPDATA%\KeyFlow\config.json)
- src-tauri/src/fdb.rs - FastKeys .fdb importer
- src-tauri/src/main.rs - Tauri setup, windows, tray, commands
- src/lib - Svelte UI (Palette, Manager, Confirm, api helpers)

## Behavior that must not regress

The matcher is a verbatim record of typed characters. Keep it that way when touching engine.rs:

- every character, spaces included, is appended to the buffer; anything typed between trigger characters breaks a match
- the buffer is cleared by Esc, Enter, Tab, arrows, Home/End, PgUp/PgDn and Delete, and reset by a pause of about 2.5s, by any mouse click, and when the focused window changes
- Backspace pops the last buffered character
- Ctrl/Alt/Win combinations never enter the buffer
- trigger modes: Auto expands on match, Confirm parks the match and waits for Enter (Esc cancels, any other key keeps typing), Manual never fires from typing

## Testing

There is no test suite besides the build. Verify changes by hand on Windows and say what you tested:

- auto, confirm and manual trigger modes, including the confirm prompt with Enter, Esc and an unrelated key
- a trigger typed with a space inside it (must not fire), and one typed after a pause or a mouse click (must not fire)
- tokens: {tab}, {enter}, {ctrl+a}, {sleep 250}, {lbutton}
- the palette: Ctrl+Space opens it, search filters, Enter types into the previous window, Ctrl+Enter copies, right-click menu, Shift+F10
- the editor: drag a token from Insert action onto the field, Ctrl+Z and Ctrl+Y, the Sensitive flag blurring values in the palette and the list
- scripts/e2e-test.ps1 drives a WinForms window and prints what an expansion produced; it expects snippets named kftest1 and kftest2, documented in the header of that file

## Pull requests

Keep changes focused, and describe both the behavior change and how you verified it on Windows. Do not commit personal configuration or data: snippets live in %APPDATA%\KeyFlow and the log is never tracked. New dependencies need a reason - the crate surface is deliberately small (Tauri, serde, the clipboard plugin).

Do not add telemetry, analytics or any network call. KeyFlow is local-only by design.
