# todowrite

Update the session todo list incrementally. Each item has a stable id so it can be added, updated, or removed across calls. The list is shown to the user inline in the chat and persisted across restarts.

## Does

- Apply one or more operations in a single call: `add` new items, `update` existing items by id, `remove` items by id, or `clear: true` to wipe the list. `clear` and `remove` run first, so a later `add` in the same call can reuse a dropped id.
- Validate every item: non-empty `id` (max 64 chars) and `content` (max 200 chars), `status` in `pending` / `in_progress` / `completed` / `cancelled`, `priority` in `0`-`10` (defaults to `5`).
- Cap the active list at `64` items. Excess returns an error rather than truncating.
- Persist the new state and append one `todos_history` event (capped at 50) per non-no-op call. The history survives restarts and is what makes "was the clear done?" answerable after reload.
- Emit a `todo` chunk so the chat UI updates inline as the model works through the plan.
- Return a TOON summary with `status: "ok" | "noop" | "error"`, `count`, and a one-line `summary` (e.g. `2 pending, 1 in_progress, 3 completed`).
- Treat `{}` (empty args) as a no-op. The active list is NOT changed, no history event is appended, the response is `status: "noop"`. The empty list does NOT mean clear.

## Does not

- Replace or rewrite the whole list. Use `clear: true` followed by `add: [...]` when you really need to start over.
- Edit files. Use `edit` or `write`.
- Track per-message todos. The active list is session-scoped, the history is session-scoped.
- Expose the list to other sessions.
- Require user confirmation. The tool always runs and persists.

## Options

| Name                | Type    | Required | Default   | Notes                                                                                                                    |
| ------------------- | ------- | -------- | --------- | ------------------------------------------------------------------------------------------------------------------------ |
| `add`               | array   | no       | `[]`      | Items to create. Each must have a unique stable `id`. `status` and `priority` are optional and default to `pending`/`5`. |
| `add[].id`          | string  | yes      | -         | Stable id. Use a short slug like `step-1` or a content hash. Max 64 chars. Cannot change later.                          |
| `add[].content`     | string  | yes      | -         | Short description. Trimmed. Non-empty after trim. Max 200 chars.                                                         |
| `add[].status`      | string  | no       | `pending` | One of `pending`, `in_progress`, `completed`, `cancelled`.                                                               |
| `add[].priority`    | integer | no       | `5`       | `0`-`10`. Higher = more important.                                                                                       |
| `update`            | array   | no       | `[]`      | Items to update by id. Only the fields you include change; the rest stay as they were.                                   |
| `update[].id`       | string  | yes      | -         | Id of an existing item. Unknown ids return an error.                                                                     |
| `update[].content`  | string  | no       | -         | New content. Same validation as `add`.                                                                                   |
| `update[].status`   | string  | no       | -         | New status.                                                                                                              |
| `update[].priority` | integer | no       | -         | New priority `0`-`10`.                                                                                                   |
| `remove`            | array   | no       | `[]`      | Ids of items to delete. Unknown ids are silently ignored.                                                                |
| `clear`             | boolean | no       | `false`   | `true` wipes the active list. `false` is a no-op. Empty args is also a no-op.                                            |

At least one of `add`, `update`, `remove`, or `clear` (set to `true`) must be present.

## Response

| Field     | Type   | Notes                                                                    |
| --------- | ------ | ------------------------------------------------------------------------ |
| `status`  | string | `ok` if any operation ran, `noop` if nothing changed, `error` otherwise. |
| `count`   | int    | Number of items in the active list after the call.                       |
| `summary` | string | Counts by status, e.g. `2 pending, 1 in_progress, 3 completed`.          |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `todowrite: <serde error>`: argument shape did not match the schema.
- `todowrite every item needs a non-empty \`id\`.`: missing or whitespace `id`.
- `todowrite item id exceeds 64 chars.`: id too long.
- `todowrite every item needs a non-empty \`content\`.`: blank content after trim.
- `todowrite item content exceeds 200 chars.`: item too long.
- `todowrite priority must be 0-10, got <n>.`: priority out of range.
- `todowrite: id \`<id>\` already exists. Use \`update\` instead of \`add\`.`: duplicate id on add.
- `todowrite update entries need a non-empty \`id\`.`: update with blank id.
- `todowrite: cannot update unknown id \`<id>\`.`: update for an id that does not exist.
- `todowrite update content cannot be empty.` / `... exceeds 200 chars.`: update with bad content.
- `todowrite update priority must be 0-10.`: update with bad priority.
- `todowrite accepts at most 64 items, got <n>.`: post-operation list too long.
- `todowrite needs the desktop shell.`: `ToolContext.app` is `None` (test context).
- `todowrite needs an active session id.`: tool ran without a session id.
- `<SessionError>`: I/O error reading or writing the session record.

## Source

`src-tauri/src/tools/todo.rs` - entry point: `TodoTool::execute()`. Persistence helpers live in `src-tauri/src/sessions.rs::append_session_todo_event` (active list + history) and `src-tauri/src/sessions.rs::read_session_todos` (read for next call).
