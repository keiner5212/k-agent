# list_directory

List directory entries. Optional recursive walk with bounded depth and parallel workers.

## Does

- Reads a directory (default: workspace root) and returns entries sorted by name.
- Skips a hardcoded list of noisy directories (`node_modules`, `.git`, `target`, `dist`, `build`, `venv`, `.venv`, ...). Also skips hidden entries (names starting with `.`).
- Caps total output at 5000 rendered lines; emits a `(truncated at 5000 lines)` note when that happens.
- Caps each individual directory read at 2000 entries.
- Caches directory reads in process for `1 second` (`CACHE_TTL`). A second call within the TTL returns the cached snapshot without re-stat-ing.
- With `recursive: true`, walks the tree in parallel using `min(configured_parallelism, host_logical_cpus, 16)` workers. Workers run in `thread::scope` chunks.
- Clamps `maxDepth` to `1..=10`. The default is `3`.
- Resolves symlinks for the `is_dir` flag without following the link during the recursive walk.

## Does not

- Show hidden files or `noise/`-style directories. Both are filtered out by name.
- Traverse further than `maxDepth` lets it. The walk stops at the depth bound.
- Delete the recursive cache TTL. The 1-second cache is intentional; expect re-reads on every burst.
- Walk a directory outside the workspace without an allow answer. The tool waits on that prompt.
- Report file sizes or mtimes. Each line is `[dir]` or `[file]` plus the name, indented when the walk is recursive.

## Options

| Name        | Type    | Required | Default        | Notes                                                                      |
| ----------- | ------- | -------- | -------------- | -------------------------------------------------------------------------- |
| `dirPath`   | string  | no       | workspace root | Absolute or workspace-relative directory.                                  |
| `recursive` | boolean | no       | `false`        | When `true`, walks the sub-tree under `dirPath`.                           |
| `maxDepth`  | integer | no       | `3`            | Recursion bound. Clamped to `1..=10`. Ignored when `recursive` is `false`. |

## Response

| Field     | Type   | Notes                                                                                           |
| --------- | ------ | ----------------------------------------------------------------------------------------------- |
| `path`    | string | Workspace-relative directory that was listed.                                                   |
| `summary` | string | Counts, for example `2 directories, 5 files`.                                                   |
| `entries` | string | One `[dir] name` or `[file] name` per line. Recursive rows stay indented by 2 spaces per level. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `list_directory \`dirPath\` must be a non-negative integer.`/`out of range.`: invalid `maxDepth`.
- `Directory not found: <resolved path>`
- `Path is not a directory: <resolved path>`
- `Unable to stat \`<path>\`: <io error>`/`Unable to read: <io error>`

## Source

`src-tauri/src/tools/list_directory.rs` - entry point: `ListDirectoryTool::execute()`. Recursion + parallelism in `walk_recursive()` / `walk_inner()`. Worker cap logic in `resolve_parallel_cap()`.
