# Security and privacy

KeyFlow is a local-only desktop application. It registers global keyboard and mouse hooks to detect triggers, keeps a small in-memory buffer of recently typed characters for matching, and writes only under %APPDATA%\KeyFlow. There is no telemetry, no analytics and no network request.

Worth knowing:

- Snippets are stored in plain text in %APPDATA%\KeyFlow\config.json. The per-snippet Sensitive flag blurs values in the UI for screenshots and recordings; it is not encryption.
- Copying a snippet uses the Windows clipboard, and the paste typing mode temporarily places text on the clipboard and restores the previous contents afterwards.
- Expansion history is not stored. last-expansion.txt holds only the mode, character counts and duration.

To report a security issue, please use GitHub private vulnerability reporting on this repository instead of a public issue.
