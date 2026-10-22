# edit

Replace an exact span in an existing file. Two shapes. Never both in one call.

## Does

- One replacement: only `filePath`, `oldString`, `newString`, and optional `replaceAll`. Do not send `edits`.
- Many replacements: only `edits`. Each item has its own `filePath`, `oldString`, `newString`, and optional `replaceAll`. Do not also send top-level `filePath`, `oldString`, `newString`, or `replaceAll`. Top-level `filePath` is not a directory for the array.
- Both shapes in one call write nothing.
- `oldString` and `newString` are exact file text. Not a line-number prefix from `read`. Not a diff hunk. Include the indentation that is in the file.
- Copy `oldString` from a `read` of that file in this turn, large enough to be unique, small enough to be exact.
- `newString` is the full replacement of that span, not a patch line with `+` or `-`.
- Fails if `oldString` is missing or appears more than once, unless `replaceAll` is true. The error says which.
- Checks every edit before any file is written. A bad item leaves every file unchanged. A failed write rolls back files already written in that call.
- Matches `oldString` as an exact substring. One pass. No fuzzy, trimmed, or similarity fallback.
- Converts `oldString` and `newString` to the file line ending (`\n` or `\r\n`) before the match.
- Locks the target file with a per-path mutex so concurrent edit/write calls cannot interleave.
- Captures before/after snapshots to `sessions/{id}/files/{callId}.before` and `{callId}.after`.
- Emits `added` and `removed` as the changed lines in this call. Several edits in one file count each change.
- `count` is present only when the call writes more than one file.
- Paths outside the workspace wait for the user.

## Does not

- Create a new file. Use `write` if the file is missing.
- Accept empty `oldString`. Use `write` for a full-file replacement.
- Forgive whitespace or indentation drift.
- Guess a nearby span. A miss returns `Could not find oldString`.
- Accept both shapes in one call.

## Options

| Name         | Type    | Required | Default | Notes                                                                                                          |
| ------------ | ------- | -------- | ------- | -------------------------------------------------------------------------------------------------------------- |
| `filePath`   | string  | no       | -       | One replacement only. Omit when `edits` is set. Not a directory for the array.                                 |
| `oldString`  | string  | no       | -       | One replacement only. Exact file text. Not a line-number prefix. Not a diff hunk. Include indentation.         |
| `newString`  | string  | no       | -       | One replacement only. Full replacement of that span. Not a patch line with `+` or `-`.                         |
| `replaceAll` | boolean | no       | `false` | One replacement only. When `true`, replace every match. Omit when `edits` is set.                              |
| `edits`      | array   | no       | -       | Many replacements only. Each item has its own `filePath`, `oldString`, `newString`, and optional `replaceAll`. |

### `edits` item

| Name         | Type    | Required | Default | Notes                                                 |
| ------------ | ------- | -------- | ------- | ----------------------------------------------------- |
| `filePath`   | string  | yes      | -       | File for this item. Not inherited from the top level. |
| `oldString`  | string  | yes      | -       | Exact file text for this item.                        |
| `newString`  | string  | yes      | -       | Full replacement of that span.                        |
| `replaceAll` | boolean | no       | `false` | When `true`, replace every match of this item.        |

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
