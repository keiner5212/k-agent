# edit

Replace an exact string in an existing file with multiple relaxations for whitespace, indentation, and escapes.

## Does

- Replaces the first (or every) match of `oldString` with `newString` in the target file.
- Tries 9 replacers in order when an exact match fails: `simple`, `line_trimmed`, `block_anchor`, `whitespace_normalized`, `indentation_flexible`, `escape_normalized`, `trimmed_boundary`, `context_aware`, `multi_occurrence`. Each attempts to find `oldString` under looser matching.
- Detects whether the file uses `\n` or `\r\n` line endings and converts `oldString` / `newString` to match before applying the replacement.
- Locks the target file with a per-path mutex for the duration of the replacement so concurrent edit/write calls cannot interleave.
- Captures before/after snapshots to `sessions/{id}/files/{callId}.before` and `{callId}.after`.
- Emits `added` and `removed` line counts from a hunk-style diff.

## Does not

- Create a new file. Use `write` if the file is missing.
- Edit empty `oldString`. The tool errors out to avoid accidental whole-file rewrites; use `write` for that intent.
- Allow whitespace to differ freely. The 9 replacers cover common drift (trailing whitespace, mixed line endings, escape-vs-raw) but a heavily reformatted file may still fail. Re-read with `read` and pass the exact bytes back.
- Refuse an over-broad match. The tool returns an error (`Refusing replacement because the matched span is much larger than oldString ...`) when the relaxed candidate grows past the disproportionality threshold rather than silently rewriting the file.

## Options

| Name         | Type    | Required | Default | Notes                                                                            |
| ------------ | ------- | -------- | ------- | -------------------------------------------------------------------------------- |
| `filePath`   | string  | yes      | -       | Absolute or workspace-relative path to an existing file.                         |
| `oldString`  | string  | yes      | -       | Substring to find. Must be non-empty and different from `newString`.             |
| `newString`  | string  | yes      | -       | Replacement body.                                                                |
| `replaceAll` | boolean | no       | `false` | When `true`, replace every match. Otherwise the tool errors on multiple matches. |

## Response

| Field     | Type    | Notes                                     |
| --------- | ------- | ----------------------------------------- |
| `path`    | string  | Workspace-relative path that was edited.  |
| `status`  | string  | Always `ok` on success.                   |
| `added`   | integer | Lines added by the edit (display-only).   |
| `removed` | integer | Lines removed by the edit (display-only). |

The TOON response body is `path` + `status` + `added` + `removed` (the integer fields are display-only and absent from the wire text; the runtime surfaces them in the frontend `ToolDisplay`).

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `edit tool requires a string ...`: missing argument.
- `No changes to apply: oldString and newString are identical.`
- `oldString cannot be empty. Use write for an intentional full-file replacement.`
- `File not found: <path>`
- `Cannot edit binary file: <path>`
- `Refusing replacement because the matched span is much larger than oldString ...`
- `Found multiple matches for oldString. Provide more surrounding context ..., or set replaceAll to true.`
- `Could not find oldString in the file. It must match exactly, including whitespace, indentation, and line endings.`

## Source

`src-tauri/src/tools/edit.rs` - entry point: `EditTool::execute()`. Replacement algorithm lives in `replace()` with 9 replacer functions.
