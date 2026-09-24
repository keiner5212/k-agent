# grep

Search file contents with the ripgrep engine compiled into the app. The job count is the configured worker-core limit, clamped to `1..=16`.

## Does

- Search a pattern with no regex metacharacters as a literal byte scan. A pattern that contains `. * + ? ( ) | [ ] { } ^ $` or `\` stays on the regex engine. There is no system `rg` binary.
- Walk files on a pool sized to the chat request parallelism.
- Default `path` to the workspace root. Absolute and workspace-relative paths are accepted.
- Treat `glob` as an include filter. `*.md` returns only markdown files. `*.{rs,toml}` is one glob. A leading `!` excludes (`!*.json`).
- Honor `caseInsensitive`. Default is case-sensitive. `token` does not match `TOKEN` unless the flag is true.
- Count each matching line once. A repeated pattern on one line is one hit. `count` is that total, not the sample length.
- Return a sample in `matches`: at most 20 lines from each file and 100 lines overall. `truncated` is `true` when `count` is larger than the sample.
- Skip `.git`, `node_modules`, `target`, and `dist`. Hidden files and gitignore rules still apply.
- Clip each sample line at 400 characters. Sort the sample by path, then line.
- Return `count: 0`, `truncated: false`, and an empty `matches` block when nothing matches.
- Ask before searching outside the workspace, same as `read`.

## Does not

- Stop the walk at 100 matches. The walk finishes so `count` stays exact. Only the printed sample is capped.
- Shell out to a system `rg` or to `bash`.
- Offer word-boundary, count-only, or files-only flags. Use a tighter `pattern`, or `bash` when the full line list must be streamed.
- Follow symlinks beyond the `ignore` crate default.
- Return binary matches. The searcher skips binary files.
- Use more cores than the settings limit.
- Cache results. Each call walks again.

## Options

| Name              | Type    | Required | Default        | Notes                                              |
| ----------------- | ------- | -------- | -------------- | -------------------------------------------------- |
| `pattern`         | string  | yes      | -              | Regex. Case-sensitive unless `caseInsensitive`.    |
| `path`            | string  | no       | workspace root | File or directory. Absolute or workspace-relative. |
| `glob`            | string  | no       | -              | Include filter. A leading `!` excludes.            |
| `caseInsensitive` | boolean | no       | false          | Match letters regardless of case.                  |

## Response

| Field       | Type    | Notes                                                   |
| ----------- | ------- | ------------------------------------------------------- |
| `pattern`   | string  | The pattern that was searched.                          |
| `count`     | integer | Matching lines, including lines left out of the sample. |
| `truncated` | string  | `true` when `matches` is a sample of a larger `count`.  |
| `matches`   | string  | `path:line:text` sample. Empty when nothing matched.    |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `grep requires a string pattern.`: `pattern` was missing or not a string.
- `grep pattern is empty.`: `pattern` was blank after trim.
- `grep: <regex error>`: the pattern is not a valid regex.
- `grep: <glob error>`: `glob` could not be compiled.
- `grep outside the workspace must run on the async dispatch path.`: the sync path will not prompt.
- `User denied access outside the workspace.`: the user denied the outside-workspace prompt.

## Source

`src-tauri/src/tools/grep.rs` - entry point: `spec()` / `execute_async()`.
