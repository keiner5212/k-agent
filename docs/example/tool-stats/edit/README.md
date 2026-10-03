# edit

Replace an exact string in an existing file. Whitespace and indentation must match.

## Does

- Replaces the first (or every) match of `oldString` with `newString` in the target file.
- One call uses one shape. Single edit: top-level `filePath`, `oldString`, `newString`. Several edits: only `edits`, and each item has its own `filePath`. Both shapes in one call is an error and writes nothing. Top-level `filePath` is not a parent of `edits`.
- Applies several edits in one call through `edits`. Every edit is checked in memory first. A failed check writes nothing. A failed write rolls back files already written in that call.
- Matches `oldString` as an exact substring. One pass. No fuzzy, trimmed, or similarity fallback. That keeps the call on a byte find instead of a similarity scan.
- Detects whether the file uses `\n` or `\r\n` line endings and converts `oldString` / `newString` to match before applying the replacement.
- Locks the target file with a per-path mutex for the duration of the replacement so concurrent edit/write calls cannot interleave.
- Captures before/after snapshots to `sessions/{id}/files/{callId}.before` and `{callId}.after`.
- Emits `added` and `removed` as the changed lines in this call. Several edits in one file count each change, not the span from the first change to the last.
- Groups edits by resolved path. `count` is that file count, and only when more than one file is written.

## Does not

- Create a new file. Use `write` if the file is missing.
- Edit empty `oldString`. The tool errors out to avoid accidental whole-file rewrites; use `write` for that intent.
- Forgive whitespace, indentation, or escape drift. Re-read with `read` and pass the exact file text.
- Guess a nearby span. A miss returns `Could not find oldString`.
- Accept top-level `filePath`, `oldString`, or `newString` together with `edits`. Pick one shape.

## Options

| Name         | Type    | Required | Default | Notes                                                                               |
| ------------ | ------- | -------- | ------- | ----------------------------------------------------------------------------------- |
| `filePath`   | string  | no       | -       | Single-edit shape only. Not a parent path for `edits`.                              |
| `oldString`  | string  | no       | -       | Single-edit shape only. Substring to find. Omit when `edits` is set.                |
| `newString`  | string  | no       | -       | Single-edit shape only. Replacement body. Omit when `edits` is set.                 |
| `replaceAll` | boolean | no       | `false` | Single-edit shape only. When `true`, replace every match. Omit when `edits` is set. |
| `edits`      | array   | no       | -       | Multi-edit shape only. Each item has its own `filePath`. Omit the top-level fields. |

## Response

| Field     | Type    | Notes                                                             |
| --------- | ------- | ----------------------------------------------------------------- |
| `path`    | string  | Workspace-relative path that was edited.                          |
| `status`  | string  | Always `ok` on success.                                           |
| `added`   | integer | Changed lines added in this call.                                 |
| `removed` | integer | Changed lines removed in this call.                               |
| `count`   | integer | File count. Present only when the call writes more than one file. |

The TOON response body is `path` + `status` + `added` + `removed`. A single-file call has no `count` field. When a language server publishes diagnostics, a `diagnostics` block is appended: `none`, or one line per note. The chat row shows the same notes.

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `edit accepts either filePath/oldString/newString or edits, not both.`
- `edit tool requires a string ...`: missing argument.
- `No changes to apply: oldString and newString are identical.`
- `oldString cannot be empty. Use write for an intentional full-file replacement.`
- `File not found: <path>`
- `Cannot edit binary file: <path>`
- `Found multiple matches for oldString. Provide more surrounding context ..., or set replaceAll to true.`
- `Could not find oldString in the file. It must match exactly, including whitespace, indentation, and line endings.`
- A whitespace or indent mismatch is a miss. Re-read the file and copy the exact span.

## Source

`src-tauri/src/tools/edit.rs` - entry point: `EditTool::execute()`. Replacement lives in `replace()`.
