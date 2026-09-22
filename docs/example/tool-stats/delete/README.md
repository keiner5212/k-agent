# delete

Delete a single file or empty directory. Captures removed line counts so the chat context walk can decrement.

## Does

- Removes a regular file with `fs::remove_file`. Reports `linesRemoved` (line count of the deleted file) so the chat context counter can subtract it.
- Removes an empty directory with `fs::remove_dir`. Refuses non-empty directories with an explicit "Directory is not empty" error.
- For targets outside the workspace, asks the user to confirm via `ask_user` before deleting.
- Reports the deletion kind (`file` or `directory`) in the response.

## Does not

- Recurse into non-empty directories. Use the shell through `ask_user` + the OS if you really need recursive deletion; that combination is not part of this tool.
- Move to a trash / recycle bin. Deletes are permanent.
- Delete when the path is outside the workspace without explicit user confirmation.
- Follow and delete symlinks. `fs::symlink_metadata` is used so the link itself is removed, not the target.
- Return any body for the deleted content. Once gone, gone.

## Options

| Name   | Type   | Required | Default | Notes                                                                                       |
| ------ | ------ | -------- | ------- | ------------------------------------------------------------------------------------------- |
| `path` | string | yes      | -       | Absolute or workspace-relative path. Outside-workspace paths trigger a confirmation prompt. |

## Response

| Field          | Type    | Notes                                                                                 |
| -------------- | ------- | ------------------------------------------------------------------------------------- |
| `path`         | string  | Workspace-relative path that was deleted.                                             |
| `status`       | string  | Always `deleted` on success.                                                          |
| `kind`         | string  | `file` or `directory`.                                                                |
| `linesRemoved` | integer | Only present when `kind` is `file`. Surfaced in the chat UI for token-budget updates. |

The TOON response body is `path` + `status` + `kind` + (when file) `linesRemoved`.

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `delete tool requires a string \`path\`.`/`delete tool \`path\` is empty.`: argument missing or blank.
- `delete outside the workspace must run on the async dispatch path.`: sync dispatch path was used for an out-of-workspace path; only the async path handles the confirmation.
- `User denied the destructive action.`: outside-workspace path and the user cancelled the confirmation dialog.
- `Path not found: <path>`
- `Directory is not empty: <path> (recursive deletion is not supported)`
- `Unable to stat \`<path>\`: <io error>`/`Unable to delete \`<path>\`: <io error>`

## Source

`src-tauri/src/tools/delete.rs` - sync entry: `DeleteTool::execute()`. Async entry: `execute_async()`. Confirmation flow: `confirm_destructive()`.
