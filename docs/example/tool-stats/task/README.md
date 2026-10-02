# task

Run one saved agent on a read-only side task and return its final text.

## Does

- Takes `agent`, `prompt`, and a 3 to 5 word `description`.
- `build` and `plan` are builtin. Any other name is a saved agent under `~/.k-agent/agents`.
- The child keeps that agent's skills and personality.
- The child's tools are the read-only subagent set: `skill`, `read`, `list_directory`, `grep`, `lsp`, `internet_search`, `fetch_url`.
- Tools saved on the agent apply only when that agent is the main agent.
- The child starts with a fresh prompt. It does not see the parent transcript.
- It does not inherit the parent system prompt.
- Returns the child's full final text.
- A question in that text is for the parent. The parent asks the user or decides, then calls `task` again.
- A saved agent is matched by name or id, case ignored.
- The child does not receive MCP tools.
- The child transcript is its own session. Open it from the task row. It is not in the sidebar.
- The transcript shows the parent assignment, the child's thinking, and its tool calls.
- Deleting the parent chat deletes that child session.
- The model is the subagent model setting, or the parent model when that setting is empty.

## Does not

- Ask the user, edit the plan, write files, or run a command that changes state. The error is `subagent is read-only.`
- Start another task from inside a task. The error is `task cannot start another task.`
- Use the tool list saved for main mode.
- Mix the child's tokens into the parent chat. The parent row stays one result. Open the child transcript to watch tokens and thinking while it runs.
- Replace one `read` or one `grep`. Those stay direct calls.
- Run without a chat session. The example harness returns that error.
- Write session checkpoints. The child does not mutate files.

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
- `subagent is read-only. report this to the parent. the parent asks the user, updates the plan, and does the work.`
- A prompt over 12000 characters is rejected before any model call.

## Source

`src-tauri/src/tools/task.rs` - `execute_async()`. The nested loop is `chat::run_task()`.
