# list_directory

List directory entries. Optional recursive walk with bounded depth and parallel workers.

## Does

- Reads a directory (default: workspace root) and returns entries sorted by name.
- Skips dependency, cache, and build directories (`node_modules`, `.git`, `target`, `dist`, `build`, `vendor`, virtualenvs, ...). Still lists `.github` and `.agents`. Other names starting with `.` are skipped.
- Caps total output at 5000 rendered lines; emits a `(truncated at 5000 lines)` note when that happens.
- Caps each individual directory read at 2000 entries.
- Caches directory reads in process for `1 second` (`CACHE_TTL`). A second call within the TTL returns the cached snapshot without re-stat-ing.
- With `recursive: true`, walks the tree in parallel using `min(configured_parallelism, host_logical_cpus, 16)` workers. Workers run in `thread::scope` chunks.
- With `glob`, returns sorted file paths only. `glob` is an argument of this tool, not a separate tool. `*.rs` matches the file name at any depth. `src/**/*.ts` matches that prefix. `*.{ts,tsx}` matches either suffix. At most 200 paths. Default depth becomes 10.
- A `glob` walk uses the same 1-second directory cache and the same worker cap as the tree walk.
- Clamps `maxDepth` to `1..=10`. The default is `3`. A `glob` with no `maxDepth` uses `10`.
- Resolves symlinks for the `is_dir` flag without following the link during the recursive walk.
- This is the directory listing. Do not use `ls`, `find`, or `tree` in `bash`.

## Does not

- Show hidden names other than `.github` and `.agents`. Dependency and build directories are filtered out by name.
- Traverse further than `maxDepth` lets it. The walk stops at the depth bound.
- Delete the recursive cache TTL. The 1-second cache is intentional; expect re-reads on every burst.
- Walk a directory outside the workspace without an allow answer. The tool waits on that prompt.
- Report file sizes or mtimes. Without `glob`, each line is `[dir]` or `[file]` plus the name, indented when the walk is recursive. With `glob`, each line is a relative path.
- Return directories from `glob`. Only files match. `*` stays inside one path segment. `**` crosses segments.
- Sort a truncated glob as a full-tree ranking. The walk stops after 200 file hits, then sorts that set.

## Options

| Name        | Type    | Required | Default        | Notes                                                                     |
| ----------- | ------- | -------- | -------------- | ------------------------------------------------------------------------- |
| `dirPath`   | string  | no       | workspace root | Absolute or workspace-relative directory.                                 |
| `recursive` | boolean | no       | `false`        | When `true`, walks the sub-tree under `dirPath`.                          |
| `maxDepth`  | integer | no       | `3`            | Recursion bound. Clamped to `1..=10`. Default is `10` when `glob` is set. |
| `glob`      | string  | no       | -              | Argument of this tool, not a separate tool. Paths only, capped at 200.    |

## Response

| Field     | Type   | Notes                                                                                         |
| --------- | ------ | --------------------------------------------------------------------------------------------- |
| `path`    | string | Workspace-relative directory that was listed.                                                 |
| `summary` | string | Counts, for example `2 directories, 5 files`.                                                 |
| `glob`    | string | Present only when `glob` was set. The pattern that was applied.                               |
| `entries` | string | One `[dir] name` or `[file] name` per line, or one relative path per line when `glob` is set. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `list_directory \`maxDepth\` must be a non-negative integer.`/`out of range.`: invalid `maxDepth`.
- `list_directory \`glob\` must be a string.`
- `list_directory \`glob\` exceeds 200 chars.`
- `Directory not found: <resolved path>`
- `Path is not a directory: <resolved path>`
- `Unable to stat \`<path>\`: <io error>`/`Unable to read: <io error>`

## Source

`src-tauri/src/tools/list_directory.rs` - entry point: `ListDirectoryTool::execute()`. Tree walk is `walk_inner()`. Glob walk is `collect_glob()`.
