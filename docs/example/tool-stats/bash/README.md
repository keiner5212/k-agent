# bash

Run one shell command in the workspace. `description` is required. The command string is split into commands and checked against the settings allow list and block list before it starts.

## Does

- Run the command with the configured shell program, or `/bin/sh -c` when that setting is empty (`cmd.exe /c` on Windows).
- Take `PATH` and the rest of the environment from that shell's login interactive startup (`-lic`), once per process. `sh` and `dash` probe `bash` when `/bin/bash` or `/usr/bin/bash` exists. Directories already on the desktop process `PATH` are kept. A failed probe is cached and the desktop environment is used. `background` uses the same environment. The user command then runs with `-c`. It is not placed inside `.bashrc` or `.zshrc`.
- Require `description` on every call: one short sentence, what the command does and why. Safe, allowed, and blocked commands still need it.
- Split a compound on `&&`, `||`, `;`, `|`, newlines, `$()`, and backticks. Single and double quotes hide separators. `bash -c`, `sh -c`, `zsh -c`, and `-lc` classify the inner string too.
- Block the call when the full string, or any one segment, is an exact `blockedCommands` entry.
- Skip the prompt when the full string is an exact `allowedCommands` entry, or every segment is.
- Run a command that is not listed and does not look dangerous.
- Ask when any segment looks destructive, hits a non-local network, or writes a file. `2>&1` and a redirect to `/dev/null` do not ask.
- Run one or more `curl` commands against `localhost`, `127.0.0.1`, or `::1` with no prompt when every flag is a local read flag (silent, head, fail, timeout, header, literal body, output to `/dev/null`). The process is started as `curl -q -g` with proxy variables cleared. Any other flag, a config file, a proxy, an upload, a redirect, or a mix with another command asks first.
- Treat `Accept for this chat` as a session grant for later dangerous commands. The block list still wins.
- Cap combined stdout and stderr at `50000` chars and stop the process after `30` seconds.
- Return `status`, `exitCode`, and `output`.
- Refuse a background job (`&`, `nohup`, `disown`) before any prompt.
- Still run an exact allow-list match, even when that string would otherwise look like a background job.

## Does not

- Run a command outside the workspace directory. The working directory is the workspace.
- Start a second shell around the configured program. The command is one argument to `-c` or `/c`.
- Run the user command inside the login shell. Startup only prints the environment, once per process.
- Match the allow or block list by prefix or regex. The settings copy says the full string.
- Treat `description` as shell. It is not executed and it is not matched against either list.
- Persist output. The result is the tool response only.
- Bypass a block list entry because the session already allowed dangerous commands.
- Start a background job. A single `&`, `nohup`, or `disown` returns an error. `&&` and `2>&1` still follow the normal rules. A process that must stay up uses `background`.
- Kill a pid that `background` still owns. That process stops when the turn ends.
- List, read, or search files. Use `glob`, `read`, and `grep`.
- Create, change, or delete files. Use `write`, `edit`, `create_folder`, and `delete`.

## Options

| Name          | Type   | Required | Default | Notes                                                                       |
| ------------- | ------ | -------- | ------- | --------------------------------------------------------------------------- |
| `description` | string | yes      | -       | One short sentence. What this command does and why. Not shell.              |
| `command`     | string | yes      | -       | One shell command. Allow and block match this full string, or each segment. |

## Response

| Field      | Type    | Notes                                                                                    |
| ---------- | ------- | ---------------------------------------------------------------------------------------- |
| `status`   | string  | `ok` when the process exits 0. `error` otherwise.                                        |
| `exitCode` | integer | Process exit code. Present when the process actually ran.                                |
| `output`   | string  | Stdout, then stderr. Clipped at 50000 chars.                                             |
| `error`    | string  | Refusal or startup message. Also set when the command fails after a login probe failure. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `bash requires a string command.`: the argument was missing or not a string.
- `bash: description is required.`: `description` was missing or not a string.
- `bash: description is empty.`: `description` was blank after trim.
- `bash command is empty.`: the command was blank after trim.
- `bash refused: command is on the block list.`: exact blockedCommands match on the full string or one segment.
- `User denied the command.`: the confirm dialog was denied or skipped.
- `bash confirmation needs the desktop shell.`: a dangerous command ran without a UI.
- `bash needs a workspace.`: no workspace path was available.
- `bash could not start <shell>`: the shell program failed to spawn.
- `bash: login shell startup failed`: the profile probe failed or timed out. A later failed command includes this note. A successful command does not.
- `bash timed out after 30s.`: the process was killed at the time limit.
- `bash: a dangerous command must run on the async dispatch path.`: the sync path refused to prompt.
- `bash does not start background jobs.`: the command backgrounds with `&`, `nohup`, or `disown`.
- `bash refused: pid`: the command tried to kill a pid `background` still owns.

## Source

`src-tauri/src/tools/bash.rs` - entry point: `spec()` / `execute_async()`.
