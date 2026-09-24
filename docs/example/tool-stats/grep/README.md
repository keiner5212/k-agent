# grep

Search file contents with ripgrep. The job count is the configured worker-core limit, clamped to `1..=16`.

## Does

- Search with the `grep`, `grep-regex`, `grep-searcher`, and `ignore` crates compiled into the app. There is no system `rg` binary.
- Walk files on a pool sized to the chat request parallelism so the search uses the configured cores.
- Default `path` to the workspace root. Absolute and workspace-relative paths are accepted.
- Accept an optional `glob` such as `*.rs` or `*.{ts,tsx}`.
- Always skip `.git/**`, `node_modules/**`, and `target/**`.
- Cap each file at 20 matches and the whole response at 100 lines.
- Clip each output line at 400 characters.
- Return `count: 0` and an empty `matches` block when ripgrep exits 1 (no matches).
- Ask before searching outside the workspace, same as `read`.

## Does not

- Search with a shell pipeline. `pattern` and `path` are argv, so a leading dash cannot become a flag (`--` separates them).
- Shell out to a system `rg`. The search engine is part of the app binary.
- Follow symlinks into other trees beyond what ripgrep does by default.
- Return binary matches. Ripgrep's default binary skip stays in place.
- Use more cores than the settings limit. `list_directory` is the other tool that already spends the same pool on parallel directory walks.

## Options

| Name      | Type   | Required | Default        | Notes                                              |
| --------- | ------ | -------- | -------------- | -------------------------------------------------- |
| `pattern` | string | yes      | -              | Ripgrep regex.                                     |
| `path`    | string | no       | workspace root | File or directory. Absolute or workspace-relative. |
| `glob`    | string | no       | -              | Extra `--glob`. Noise directories stay excluded.   |

## Response

| Field       | Type    | Notes                                                            |
| ----------- | ------- | ---------------------------------------------------------------- |
| `pattern`   | string  | The pattern that was searched.                                   |
| `count`     | integer | Matches included in this response, at most 100.                  |
| `truncated` | string  | `true` when more lines existed than the cap. `false` otherwise.  |
| `matches`   | string  | `path:line:text` lines from ripgrep. Empty when nothing matched. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `grep requires a string pattern.`: `pattern` was missing or not a string.
- `grep pattern is empty.`: `pattern` was blank after trim.
- `grep: <regex error>`: the pattern is not a valid regex.
- `grep outside the workspace must run on the async dispatch path.`: the sync path will not prompt.
- `User denied access outside the workspace.`: the user denied the outside-workspace prompt.

## Source

`src-tauri/src/tools/grep.rs` - entry point: `spec()` / `execute_async()`.
