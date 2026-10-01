# todowrite

Replace the session todo list. The model sends the full list every call. There are no ids.

## Does

- Replace the active list with the `todos` array. Each item is `content`, `status`, and `priority`.
- Accept `status` of `pending`, `in_progress`, `completed`, or `cancelled`.
- Accept `priority` of `high`, `medium`, or `low`. Stored as `8`, `5`, or `2` for the chat UI.
- Keep at most one `in_progress` item. Two or more returns an error and changes nothing.
- Cap the list at `64` items. `content` max `200` chars. Excess returns an error.
- Reuse the stored id when `content` matches an existing item, so a status change stays the same row.
- Assign a new id (`t1`, `t2`, ...) for new content. The model never sends ids.
- Persist the list and one `todos_history` event (capped at 50) when the list changes.
- Emit a `todo` chunk so the latest list renders at the bottom of the chat.
- Return TOON with `status`, `count`, `summary`, and the list as `todos` without ids.
- Treat an empty `todos` array as a clear.

## Does not

- Take `id`, `add`, `update`, `remove`, or `clear`. Those arguments return the expected shape.
- Edit files. Use `edit` or `write`.
- Track per-message todos. The active list is session-scoped.
- Expose the list to other sessions.
- Require user confirmation. The tool always runs and persists.

## Options

| Name               | Type   | Required | Default | Notes                                                         |
| ------------------ | ------ | -------- | ------- | ------------------------------------------------------------- |
| `todos`            | array  | yes      | -       | Full ordered list. Empty array clears the list. Max 64 items. |
| `todos[].content`  | string | yes      | -       | Short task. Trimmed. Non-empty. Max 200 chars.                |
| `todos[].status`   | string | yes      | -       | `pending`, `in_progress`, `completed`, or `cancelled`.        |
| `todos[].priority` | string | yes      | -       | `high`, `medium`, or `low`.                                   |

## Response

| Field     | Type   | Notes                                                                      |
| --------- | ------ | -------------------------------------------------------------------------- |
| `status`  | string | `ok` when the list changed, `noop` when it did not, `error` on a bad call. |
| `count`   | int    | Items in the list after the call.                                          |
| `summary` | string | Counts by status, e.g. `1 pending, 1 in_progress`.                         |
| `todos`   | string | Numbered lines: `1. [in_progress] [high] Run tests`. No ids.               |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `todowrite takes {"todos":[...]}`: missing `todos`, or `id` / `add` / `update` / `remove` / `clear` was sent.
- `todowrite: todos[N] must be an object...`: an item is not an object.
- `todowrite: todos[N] includes id...`: an item still has an id.
- `todowrite: todos[N] is missing content/status/priority.`: a required field is absent.
- `todowrite: todos[N].content is empty.` / `... exceeds 200 chars.`: blank or too long.
- `todowrite: todos[N].status must be pending, in_progress, completed, or cancelled. Got "...".`: bad status.
- `todowrite: todos[N].priority must be "high", "medium", or "low". Got "...".`: bad priority, including a number.
- `todowrite: only one item can be in_progress.`: more than one active item. Names them.
- `todowrite accepts at most 64 items, got N.`: list too long.
- `todowrite needs the desktop shell.`: `ToolContext.app` is `None`.
- `todowrite needs an active session id.`: tool ran without a session id.

## Source

`src-tauri/src/tools/todo.rs`, entry `TodoTool::execute()`. Persistence is `append_session_todo_event` and `read_session_todos` in `src-tauri/src/sessions.rs`.
