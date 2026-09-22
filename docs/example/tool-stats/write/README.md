# write

Create or fully overwrite a file. Path is absolute or workspace-relative.

## Does

- Creates the target file, creating any missing parent directories first (`fs::create_dir_all`).
- Overwrites without diffing; the prior contents (if any) are captured as a session revision.
- Emits `added` and `removed` line counts from a hunk-style diff between the prior content and the new content, so the chat UI can show a summary.
- Captures a before/after snapshot pair to `sessions/{id}/files/{callId}.before` and `{callId}.after`. The frontend reads those for the diff modal.

## Does not

- Apply a partial edit. Use `edit` for find-and-replace changes; this tool always replaces the whole file.
- Refuse to overwrite a non-empty file. Read first if you need to preserve existing content (`read` then `edit` or a fresh `write`).
- Validate that `content` is a particular file type (no markdown lint, no TS parse check).
- Write outside the workspace.

## Options

| Name       | Type   | Required | Default | Notes                                                                |
| ---------- | ------ | -------- | ------- | -------------------------------------------------------------------- |
| `filePath` | string | yes      | -       | Absolute or workspace-relative path. Parent directories are created. |
| `content`  | string | yes      | -       | Full new file body. May be empty.                                    |

## Response

| Field     | Type    | Notes                                                                              |
| --------- | ------- | ---------------------------------------------------------------------------------- |
| `path`    | string  | Workspace-relative path that was written.                                          |
| `status`  | string  | Always `ok` on success.                                                            |
| `added`   | integer | Lines added by the write (hunk diff). Display-only; not part of the TOON response. |
| `removed` | integer | Lines removed by the write (hunk diff). Display-only.                              |

The TOON response body is two fields: `path` + `status: "ok"`.

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `write tool requires a string \`content\`.`/`write tool requires a string \`filePath\`.`
- `write tool \`filePath\` is empty.`
- `Unable to create parent directory \`...\` <io error>`.
- `Unable to write \`...\` <io error>`.

## Source

`src-tauri/src/tools/write.rs` - entry point: `WriteTool::execute()`. Before/after capture lives in `write_file_revision` (`src-tauri/src/sessions.rs`).
