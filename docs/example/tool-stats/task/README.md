# task

Run one saved agent on a side task and return its final text.

## Does

- Takes `agent`, `prompt`, and a 3 to 5 word `description`.
- `build` and `plan` are builtin. Any other name is a saved agent under `~/.k-agent/agents`.
- The child uses that agent's tools, minus `task`.
- The child starts with a fresh prompt. It does not see the parent transcript.
- The child is that agent. It loads that agent's skills, personality, and tools.
- It does not inherit the parent system prompt.
- Returns the child's full final text.
- File writes from the child land in the same session snapshots.
- `plan` cannot edit files. `build` can.
- A saved agent is matched by name or id, case ignored.
- The child does not receive MCP tools. It uses the agent's own tool list.
- `build` gets every agent tool except `task`. `plan` gets read, search, and `lsp` only.
- The child transcript is its own session. Open it from the task row. It is not in the sidebar.
- Deleting the parent chat deletes that child session.
- The model is the subagent model setting, or the parent model when that setting is empty.

## Does not

- Start another task from inside a task. The error is `task cannot start another task.`
- Mix the child's tokens into the parent chat. The parent row stays one result. Open the child transcript to watch tokens and thinking while it runs.
- Replace one `read` or one `grep`. Those stay direct calls.
- Run without a chat session. The example harness returns that error.

## Options

| Name          | Type   | Required | Default | Notes                                                 |
| ------------- | ------ | -------- | ------- | ----------------------------------------------------- |
| `description` | string | yes      | -       | 3 to 5 words.                                         |
| `prompt`      | string | yes      | -       | Full task, including what to return. Max 12000 chars. |
| `agent`       | string | yes      | -       | `build`, `plan`, or a saved agent name.               |

## Response

| Field         | Type   | Notes                   |
| ------------- | ------ | ----------------------- |
| `agent`       | string | The agent that ran.     |
| `description` | string | The short label.        |
| `status`      | string | `ok` on success.        |
| `result`      | string | The child's final text. |

See `response.toon`.

## Errors

- `task requires description, prompt, and agent.`
- `task prompt exceeds 12000 chars.`
- `task needs a chat session.`
- `task cannot start another task.`
- `No saved agent named <name>.`
- `task must run on the async path.`
- `task needs the desktop shell.`
- `task agent name is empty.`
- A prompt over 12000 characters is rejected before any model call.

## Source

`src-tauri/src/tools/task.rs` - `execute_async()`. The nested loop is `chat::run_task()`.
