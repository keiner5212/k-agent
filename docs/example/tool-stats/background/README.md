# background

Start one workspace command and keep it until the chat turn ends, then kill the process group.

## Does

- Run the command with the configured shell program, or `/bin/sh -c` when that setting is empty (`cmd.exe /c` on Windows).
- Use the same allow list, block list, and danger check as `bash`.
- Skip the prompt for a command `bash` would run immediately, including `npm run dev`.
- Ask the user to deny, allow once, or allow for this chat when the command looks destructive.
- Treat `Accept for this chat` as the same shell session grant `bash` uses. The block list still wins.
- Drain stdout and stderr so a quiet server does not block on a full pipe. The captured text is capped at `8000` chars.
- Wait `1200ms`. If the process is still running, return `status: running` and `pid`.
- If the process exits during that wait, return `status` and `exitCode` and do not track it.
- Kill the process group when the turn ends, including cancel and error. Unix sends `SIGTERM`, then `SIGKILL`, to the group. Windows uses `taskkill /F /T`.
- Return the startup output with the pid so a later `page_shot` can target the URL the server printed.

## Does not

- Wait for the process to finish. A command that should exit uses `bash`.
- Keep the process after the turn. The next user message starts a new turn with a new process.
- Accept `&`, `nohup`, or `disown` in the command. The tool owns the process lifetime.
- Run outside the workspace directory. The working directory is the workspace.
- Match the allow or block list by prefix or regex. The settings copy says the full string.
- Persist output past the tool response. Later turns do not see the server log.

## Options

| Name      | Type   | Required | Default | Notes                                                                        |
| --------- | ------ | -------- | ------- | ---------------------------------------------------------------------------- |
| `command` | string | yes      | -       | The process itself, for example `npm run dev`. No `&`, `nohup`, or `disown`. |

## Response

| Field      | Type    | Notes                                                                    |
| ---------- | ------- | ------------------------------------------------------------------------ |
| `status`   | string  | `running` while the process lives. `ok` or `error` if it already exited. |
| `pid`      | integer | Process id. Present only while `status` is `running`.                    |
| `exitCode` | integer | Present when the process exited during startup.                          |
| `output`   | string  | Stdout and stderr from the startup window. Capped at 8000 chars.         |
| `error`    | string  | Refusal or startup message. Present when the process did not start.      |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `background requires a string command.`: the argument was missing or not a string.
- `background command is empty.`: the command was blank after trim.
- `background refused: command is on the block list.`: exact blockedCommands match.
- `background keeps the process until the turn ends.`: the command used `&`, `nohup`, or `disown`.
- `User denied the command.`: the confirm dialog was denied or skipped.
- `bash confirmation needs the desktop shell.`: a dangerous command ran without a UI.
- `background needs a workspace.`: no workspace path was available.
- `background could not start`: the shell program failed to spawn.
- `background requires an active chat turn.`: the process started outside a turn and was stopped.
- `background must run on the async dispatch path.`: the sync path refused the call.

## Source

`src-tauri/src/tools/background.rs` - entry point: `spec()` / `execute_async()`. Turn cleanup: `TurnSlot`.
