# bash

Run one shell command in the workspace. The command string is checked against the settings allow list and block list before it starts.

## Does

- Run the command with the configured shell program, or `/bin/sh -c` when that setting is empty (`cmd.exe /c` on Windows). Login startup files are not loaded.
- Match `blockedCommands` and `allowedCommands` against the full trimmed command string, exactly.
- Refuse a blocked command even when the same string is also allowed.
- Run an allowed command with no prompt, including commands that would otherwise look dangerous.
- Run a command that is not listed and does not look dangerous.
- Ask the user to deny, allow once, or allow for this chat when the command looks destructive, networked, or redirects output.
- Treat `Accept for this chat` as a session grant for later dangerous commands. The block list still wins.
- Cap combined stdout and stderr at `50000` chars and stop the process after `30` seconds.
- Return `status`, `exitCode`, and `output`.
- Refuse a background job (`&`, `nohup`, `disown`) before any prompt.
- Still run an exact allow-list match, even when that string would otherwise look like a background job.

## Does not

- Run a command outside the workspace directory. The working directory is the workspace.
- Start a second shell around the configured program. The command is one argument to `-c` or `/c`.
- Match the allow or block list by prefix or regex. The settings copy says the full string.
- Persist output. The result is the tool response only.
- Bypass a block list entry because the session already allowed dangerous commands.
- Start a background job. A single `&`, `nohup`, or `disown` returns an error. `&&` and `2>&1` still follow the normal rules.

## Options

| Name      | Type   | Required | Default | Notes                                                                  |
| --------- | ------ | -------- | ------- | ---------------------------------------------------------------------- |
| `command` | string | yes      | -       | One shell command. Exact allow and block match uses this whole string. |

## Response

| Field      | Type    | Notes                                                             |
| ---------- | ------- | ----------------------------------------------------------------- |
| `status`   | string  | `ok` when the process exits 0. `error` otherwise.                 |
| `exitCode` | integer | Process exit code. Present when the process actually ran.         |
| `output`   | string  | Stdout, then stderr. Clipped at 50000 chars.                      |
| `error`    | string  | Refusal or startup message. Present when the process did not run. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `bash requires a string command.`: the argument was missing or not a string.
- `bash command is empty.`: the command was blank after trim.
- `bash refused: command is on the block list.`: exact blockedCommands match.
- `User denied the command.`: the confirm dialog was denied or skipped.
- `bash confirmation needs the desktop shell.`: a dangerous command ran without a UI.
- `bash needs a workspace.`: no workspace path was available.
- `bash could not start <shell>`: the shell program failed to spawn.
- `bash timed out after 30s.`: the process was killed at the time limit.
- `bash: a dangerous command must run on the async dispatch path.`: the sync path refused to prompt.
- `bash does not start background jobs.`: the command backgrounds with `&`, `nohup`, or `disown`.

## Source

`src-tauri/src/tools/bash.rs` - entry point: `spec()` / `execute_async()`.
