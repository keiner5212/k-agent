# todowrite

Replace the session todo list with the supplied ordered list. Single source of truth for the in-flight plan.

## Does

- Replace the session-scoped todo list atomically. The LLM sends the full ordered list every call.
- Validate every item: non-empty `content`, status in `pending` / `in_progress` / `completed` / `cancelled`, priority in `high` / `medium` / `low`.
- Cap the list at `64` items and `200` chars per item. Excess returns an error rather than truncating.
- Persist the list to `sessions/<id>/session.json` so it survives restarts.
- Emit a `todo` chunk so the chat UI updates inline as the model works through the plan.
- Return a TOON summary with `count`, `status: "ok"`, and a one-line `summary` (e.g. `2 pending, 1 in_progress, 3 completed`).

## Does not

- Edit files or run shell commands. Use `edit` / `write` / `bash`.
- Track per-message todos. The list is session-scoped, not message-scoped.
- Expose the list to other sessions.
- Require user confirmation. The tool always runs and persists.

## Options

| Name               | Type   | Required | Default | Notes                                                                          |
| ------------------ | ------ | -------- | ------- | ------------------------------------------------------------------------------ |
| `todos`            | array  | yes      | `[]`    | Full ordered list. Empty array clears the session list.                        |
| `todos[].content`  | string | yes      | -       | Short description of the task. Trimmed. Non-empty after trim. Max `200` chars. |
| `todos[].status`   | string | yes      | -       | One of `pending`, `in_progress`, `completed`, `cancelled`.                     |
| `todos[].priority` | string | yes      | -       | One of `high`, `medium`, `low`.                                                |

## Response

| Field     | Type   | Notes                                                           |
| --------- | ------ | --------------------------------------------------------------- |
| `status`  | string | Always `ok` on success.                                         |
| `count`   | int    | Number of items in the new list.                                |
| `summary` | string | Counts by status, e.g. `2 pending, 1 in_progress, 3 completed`. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `todowrite: <serde error>`: argument shape did not match the schema (missing `todos`, wrong types).
- `todowrite accepts at most 64 items.`: list exceeds the cap.
- `todowrite every item needs a non-empty \`content\`.`: blank content after trim.
- `todowrite item content exceeds 200 chars.`: item too long.
- `todowrite needs the desktop shell.`: `ToolContext.app` is `None` (test context).
- `todowrite needs an active session id.`: tool ran without a session id.
- `<SessionError>`: I/O error reading or writing the session record.

## Source

`src-tauri/src/tools/todo.rs` - entry point: `TodoTool::execute()`. Persistence helpers live in `src-tauri/src/sessions.rs::update_session_todos`.
