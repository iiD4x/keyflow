# FastKeys 5.10 - format and behavior notes

Scope: understand a FastKeys 5.10 installation well enough to (a) reproduce the parts of its text-expander behavior that KeyFlow implements and (b) import existing rules into KeyFlow. Observations, inferences and unknowns are kept separate.

## Targets and evidence

| Item | Evidence |
| --- | --- |
| Install layout | install dir contains FastKeys.exe (PE32, x86), SciLexer.dll, Resources.dll, vcruntime140.dll |
| Data folders | AppData\settings.ini, color.ini, Current.fktheme inside the install dir; phrase database under Documents\ (Settings.fdb, Examples.fdb) |
| Bundled samples | Documents\Settings.fdb, Documents\Examples.fdb and Resources\Inst\Settings.fdb were byte-identical in the copy examined, i.e. shipped samples rather than user data |
| Engine family | AutoHotkey v1: the shipped Documents\Scripts\Advanced Calculator.ahk uses v1 syntax (#NoEnv, SetWorkingDir %A_ScriptDir%, Menu commands) |
| Static analysis | PE headers and strings only; no decompilation was performed (no Ghidra/IDA provider was available in the analysis session) |
| Settings location | %APPDATA%\FastKeys\settings.ini (a single [Options] section) |
| User database location | configurable; the settings file records it in the DataFile key |

## Settings keys observed

Keys seen in %APPDATA%\FastKeys\settings.ini that describe the Text Expander module include:

- TEKeyDelay, TEPasteDelay, TEPressDuration, TEFocusDelay, TEClip - insertion timing and clipboard handling
- TETypedCase, TESpintax, TEAutoComplete, TEInclude, TEExclude, TEModifiers - behavior switches and app filters
- Startup, RunAsAdmin, Theme, AutoUpdate - application-level options
- DataFile, BFolder, DFolder, PFolder, TFolder, WFolder - data and folder locations
- Module1..5 and Active1..5 - module ordering and activation; the numbering was not resolved (see Unknowns)

## .fdb format (observed)

The database is UTF-8 text (BOM EF BB BF, CRLF newlines) that serializes a tree of items:

- First line: menu names, pipe separated (Menu 1|Menu 2)
- "‼" (U+203C) appears as a section separator
- Tree glyph lines (U+2192, U+221F) and breadcrumb lines (Default, Default|Phrase files) appear between item groups
- Item records are single lines starting with 0 or 1, with fields separated by "•" (U+2022)

Text Expander records carry 16 fields. Observed mapping for Send entries:

| Index | Meaning (observed) |
| --- | --- |
| 0 | 0/1 flag - semantics unverified |
| 1 | trigger |
| 2 | send text ({tab}/{enter} style tokens; "¶" U+00B6 decodes to a newline) |
| 3-10 | numeric or empty flags - unverified |
| 11 | display name |
| 12 | method - "Send" for text-expander entries |
| 13-15 | trailing zeros |

## Import mapping used by KeyFlow

- Accept a record only when it has at least 16 fields, field 12 is "Send" and field 1 is non-empty
- trigger = field 1 trimmed; replacement = field 2 with "¶" mapped to a newline; name = field 11
- imported entries are enabled; field 0 semantics were not resolved
- duplicates are collapsed by trigger, keeping the last occurrence in the file

## Unknowns / not verified

- Meaning of field 0 and the flags at fields 3-10
- FastKeys module numbering (Module1..5 vs Active1..5)
- No decompilation: conclusions come from shipped artifacts, the serialized format and file-level evidence
- FastKeys' own "human typing" timing was not instrumented; KeyFlow ships configurable presets instead of claiming parity
- Non-Latin triggers: KeyFlow tracks a letter/digit keystroke buffer for matching, so triggers written in other scripts are imported but will not match while typing

## How this shaped KeyFlow

- Same trigger semantics: suffix match on a rolling keystroke buffer, case-insensitive by default, backspace-undo of the typed trigger, then injection with optional per-character delays
- Token syntax accepts {tab}/{enter} style tokens as used by existing rules
- The importer implements the mapping above
