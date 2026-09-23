# read

Read a file as line-numbered text. Path is absolute or workspace-relative.

## Does

- Reads a file in `BufReader` mode and emits each requested line prefixed with its 1-based line number.
- Honors `offset` (1-based start line) and `limit` (max lines) for windowed reads on long files.
- Caps the response at `50 KB` of rendered text or 2000 lines, whichever comes first; emits a "capped at 50 KB" or "Use offset=N to continue" footer when truncated.
- Caps any single line at 2000 chars; lines longer than that are truncated with a `... (line truncated to 2000 chars)` suffix.
- Trims trailing `\r\n` / `\n` per line so the footer and line numbers stay aligned.
- Resolves fuzzy sibling suggestions (`Did you mean ...`) when the requested file is missing.
- Emits `startLine` and `endLine` matching `offset` + the number of returned lines so the chat UI can render the editor gutter.

## Does not

- Edit files. Use `edit` for in-place changes or `write` for full replacements.
- Return binary content. Files whose extension is in the binary list (`.zip`, `.png`, `.so`, `.wasm`, ...) error out before reading.
- Read directories. Use `list_directory` instead.
- Follow symlinks that resolve outside the workspace without an allow answer. Outside paths wait for the user.
- Cache file contents. Every call re-reads.

## Options

| Name       | Type    | Required | Default | Notes                                                              |
| ---------- | ------- | -------- | ------- | ------------------------------------------------------------------ |
| `filePath` | string  | yes      | -       | Absolute or workspace-relative path. Must point to a regular file. |
| `offset`   | integer | no       | `1`     | 1-based start line. Minimum `1`. Out-of-range offset errors.       |
| `limit`    | integer | no       | `2000`  | Maximum lines to return. Minimum `1`.                              |

## Response

| Field       | Type    | Notes                                                                                                                                     |
| ----------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `path`      | string  | Workspace-relative path.                                                                                                                  |
| `startLine` | integer | First line number actually emitted.                                                                                                       |
| `endLine`   | integer | Last line number actually emitted.                                                                                                        |
| `content`   | string  | Line-numbered body with a trailing `(End of file - total N lines)`, `(Showing lines A-B of N)`, or `(Output capped at 50 KB ...)` footer. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `read tool requires a string \`filePath\`.`: argument missing or wrong type.
- `read tool \`filePath\` is empty.`: argument is whitespace.
- `read tool \`offset\` must be a non-negative integer.`/`at least 1.`/`out of range.`: invalid offset.
- `File not found: <resolved path>` (optionally followed by `Did you mean one of these? ...`).
- `Path is a directory. Use list_directory.`
- `Cannot read binary file: <resolved path>` (extension-based or invalid UTF-8 bytes).
- `Offset <n> is out of range for this file (<m> lines).`
- `Unable to read \`<path>\`: <io error>`.

## Source

`src-tauri/src/tools/read.rs` - entry point: `ReadTool::execute()`. Window reading lives in `read_windowed()`.
