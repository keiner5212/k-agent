# apply_patch

Apply one patch that adds, updates, or deletes files. Every file is checked before any write.

## Does

- Accepts one `patchText` envelope from `*** Begin Patch` through `*** End Patch`.
- Adds a file with `*** Add File: path` and `+` lines.
- Updates a file with `*** Update File: path`, optional `*** Move to: path`, and one or more `@@` hunks.
- A hunk line starts with a space (context), `-` (remove), or `+` (add). Context must match once.
- Deletes a file with `*** Delete File: path`.
- Plans every file in memory. A bad hunk writes nothing.
- A failed write restores files already written in that call.
- Notifies the language server after a successful write or delete when LSP is enabled and a server is installed.
- Paths outside the workspace wait for the user.
- Keeps a trailing newline on updated files.
- A move writes the new path, then removes the old path.
- After success, both the new path and a removed path are sent to the language server.
- One file in the patch also stores a before/after snapshot on the session.

## Does not

- Fuzzy-match a hunk. Use `edit` only for one exact span, and only when the text matches.
- Create a parent that the path cannot resolve. The error names the path.
- Apply a patch larger than 200000 characters.
- Leave a partial tree when a later hunk fails. Intentional.

## Options

| Name        | Type   | Required | Default | Notes                                      |
| ----------- | ------ | -------- | ------- | ------------------------------------------ |
| `patchText` | string | yes      | -       | Full patch, including the begin/end lines. |

## Response

| Field     | Type    | Notes                                 |
| --------- | ------- | ------------------------------------- |
| `status`  | string  | `ok` on success.                      |
| `added`   | integer | Lines added across the patch.         |
| `removed` | integer | Lines removed across the patch.       |
| `count`   | integer | File count.                           |
| `files`   | string  | One workspace-relative path per line. |

See `response.toon`. When a language server publishes diagnostics for a changed file, a `diagnostics` block is appended: `none`, or one line per note. The chat row shows the same notes.

## Errors

- `apply_patch requires a string patchText.`
- `apply_patch needs *** Begin Patch.`
- `apply_patch needs *** End Patch.`
- `apply_patch patch has no file changes.`
- `apply_patch hunk matched N times.`
- `File not found: <path>`
- `File already exists: <path>`
- `apply_patch update <path> needs a @@ hunk.`
- `apply_patch hunk line in <path> must start with space, +, or -.`

## Source

`src-tauri/src/tools/apply_patch.rs` - `ApplyPatchTool::execute()` and `parse_patch()`.
