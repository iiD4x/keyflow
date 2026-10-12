# FastKeys .fdb format notes

Status: partially reverse engineered from FastKeys 5.10 files (bundled samples and a real-world database). Observations only - see docs/rea/fastkeys-5.10.md for evidence and unknowns.

## Encoding

- UTF-8 with BOM
- CRLF line endings

## Line types

1. Header: menu names separated by "|" - e.g. "Menu 1|Menu 2"
2. Separator: "‼" (U+203C)
3. Tree glyphs and breadcrumbs: lines like "→", "∟", "Default|Phrase files"
4. Item records: start with "0" or "1"; fields separated by "•" (U+2022)

## Item record layout (Send entries)

0-based field indexes for Text Expander Send entries (16 fields):

| Index | Content |
| --- | --- |
| 0 | 0/1 flag (unknown) |
| 1 | trigger |
| 2 | send text ({tab}, {enter} tokens; ¶ = newline) |
| 3-10 | numeric flags (unknown) |
| 11 | display name |
| 12 | method - "Send" identifies text-expander send entries |
| 13-15 | trailing zeros |

## Synthetic example (no user data)

    1•email1•name@example.com•0•0•0•0•0•1•0••Email address•Send•0•0•0
    1•login1•user@example.com{tab}secret{enter}•0•0•0•0•0•1•0••Portal login•Send•0•0•0

## Importing into KeyFlow

KeyFlow's importer accepts lines with at least 16 fields where field 12 is "Send" and field 1 is non-empty. "¶" is converted to a newline; {tab}/{enter} tokens are preserved and handled by KeyFlow's own tokenizer.
