# ask_user

Block a turn and ask the user one to four questions through a frontend dialog.

## Does

- Returns a `question` `ChatChunk` with the questions payload; the frontend opens `QuestionDialog` and shows each question on its own tab.
- Accepts `1..=4` questions per call. More or fewer is rejected.
- Resolves the user's reply (or cancel) into a synthetic tool result and feeds it back as a `tool` role message on the next LLM turn.
- Supports `multiSelect` per question so the user may select zero, one, or many options.
- Supports `allowFreeText` per question so the user can type a free-text answer alongside (or instead of) the option picks.
- Reports which `questionId` was answered, which option labels were picked, and any free-text value, summarized per question in the response.
- Tracks answers through a shared `AskUserRegistry` keyed by `call_id` so submit and cancel operations are race-safe.

## Does not

- Persist the answers in a session. The response is a single in-memory tool result; it is not stored on disk.
- Time out while waiting. The async dispatcher awaits the registry until the user submits or cancels, or until the chat is cancelled.
- Allow questions with fewer than 2 options. Use the chat input for free-form prompts instead.
- Run from the synchronous `Tool::execute` path. The tool errors out with `ask_user must run on the async dispatch path.` if you call it via the sync dispatcher; only the async `execute_async` implementation is wired to wait on the registry.

## Options

| Name        | Type  | Required | Default | Notes                 |
| ----------- | ----- | -------- | ------- | --------------------- |
| `questions` | array | yes      | -       | 1-4 question objects. |

### `questions[]` element shape

| Field           | Type    | Required | Default | Notes                                                            |
| --------------- | ------- | -------- | ------- | ---------------------------------------------------------------- |
| `id`            | string  | yes      | -       | Stable id used to match answers back to questions.               |
| `header`        | string  | yes      | -       | Short tab label (1-4 words).                                     |
| `question`      | string  | yes      | -       | Full question text shown above the options.                      |
| `options`       | array   | yes      | -       | At least 2 entries. Each is `{ label, description?, preview? }`. |
| `multiSelect`   | boolean | no       | `false` | Allows picking 0+ options.                                       |
| `allowFreeText` | boolean | no       | `true`  | Shows a free-text input next to the options.                     |

## Response

| Field     | Type   | Notes                                                                                                                                                                           |
| --------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `status`  | string | `ok` when the user submitted, `cancelled` if the user closed the dialog.                                                                                                        |
| `answers` | string | One bullet per question in the form `- <header>: <answer>` with `selected` joined by `, ` and any `freeText` appended as `free-text: ...`. Absent when `status` is `cancelled`. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `ask_user tool requires a \`questions\` array.`: missing or wrong type.
- `ask_user \`questions\` must contain at least one item.`: empty array.
- `ask_user \`questions\` can hold at most 4 items.`: too many.
- `ask_user failed to parse \`questions\`: <serde error>`: shape mismatch.
- `ask_user every question needs at least two options.`: any question has `<2` options.
- `ask_user must run on the async dispatch path.`: synchronous `Tool::execute` was invoked (test harness should call `execute_async` directly).

## Source

`src-tauri/src/tools/ask_user.rs` - sync entry: `AskUserTool::execute()` (always errors). Async entry: `execute_async()`. Registry in `ask_user_registry()`. Frontend submit / cancel tauri commands in the same file.
