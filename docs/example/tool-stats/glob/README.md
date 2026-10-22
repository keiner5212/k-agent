# glob

List directory entries. Optional recursive walk with bounded depth and parallel workers. Optional `pattern` filter for file names.

## Does

- Reads a directory (default: workspace root) and returns entries sorted by name.
- Skips dependency, cache, and build directories (`node_modules`, `.git`, `target`, `dist`, `build`, `vendor`, virtualenvs, ...). Still lists `.github` and `.agents`. Other names starting with `.` are skipped.
- Caps total output at 5000 rendered lines; emits a `(truncated at 5000 lines)` note when that happens.
- Caps each individual directory read at 2000 entries.
- Caches directory reads in process for `1 second` (`CACHE_TTL`). A second call within the TTL returns the cached snapshot without re-stat-ing.
- With `recursive: true`, walks the tree in parallel using `min(configured_parallelism, host_logical_cpus, 16)` workers. Workers run in `thread::scope` chunks.
- With `pattern`, returns sorted file paths only. `*.rs` matches the file name at any depth. `src/**/*.ts` matches that prefix. `*.{ts,tsx}` matches either suffix. At most 200 paths. Default depth becomes 10.
- A `pattern` walk uses the same 1-second directory cache and the same worker cap as the tree walk.
- Clamps `maxDepth` to `1..=10`. The default is `3`. A `pattern` with no `maxDepth` uses `10`.
- Resolves symlinks for the `is_dir` flag without following the link during the recursive walk.
- This is the directory listing. Do not use `ls`, `find`, or `tree` in `bash`.

## Does not

- Show hidden names other than `.github` and `.agents`. Dependency and build directories are filtered out by name.
- Traverse further than `maxDepth` lets it. The walk stops at the depth bound.
- Bypass the 1-second directory cache. A repeat call inside the TTL returns the cached snapshot.
- Walk a directory outside the workspace without an allow answer. The tool waits on that prompt.
- Report file sizes or mtimes. Without `pattern`, each line is `[dir]` or `[file]` plus the name, indented when the walk is recursive. With `pattern`, each line is a relative path.
- Return directories from `pattern`. Only files match. `*` stays inside one path segment. `**` crosses segments.
- Sort a truncated pattern match as a full-tree ranking. The walk stops after 200 file hits, then sorts that set.

## Options

| Name        | Type    | Required | Default        | Notes                                                                        |
| ----------- | ------- | -------- | -------------- | ---------------------------------------------------------------------------- |
| `dirPath`   | string  | no       | workspace root | Absolute or workspace-relative directory.                                    |
| `recursive` | boolean | no       | `false`        | When `true`, walks the sub-tree under `dirPath`.                             |
| `maxDepth`  | integer | no       | `3`            | Recursion bound. Clamped to `1..=10`. Default is `10` when `pattern` is set. |
| `pattern`   | string  | no       | -              | Return only matching file paths, capped at 200.                              |

## Response

| Field     | Type   | Notes                                                                                            |
| --------- | ------ | ------------------------------------------------------------------------------------------------ |
| `path`    | string | Workspace-relative directory that was listed.                                                    |
| `summary` | string | Counts, for example `2 directories, 5 files`, or `5 files` for a pattern walk.                   |
| `pattern` | string | Present only when `pattern` was set. The pattern that was applied.                               |
| `entries` | string | One `[dir] name` or `[file] name` per line, or one relative path per line when `pattern` is set. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `glob \`maxDepth\` must be a non-negative integer.`/`out of range.`: invalid `maxDepth`.
- `glob \`pattern\` must be a string.`
- `glob \`pattern\` exceeds 200 chars.`
- `Directory not found: <resolved path>`
- `Path is not a directory: <resolved path>`
- `Unable to stat \`<path>\`: <io error>`/`Unable to read: <io error>`

## Source

`src-tauri/src/tools/glob.rs` - entry point: `GlobTool::execute()`. Tree walk is `walk_inner()`. Pattern walk is `collect_pattern()`.
