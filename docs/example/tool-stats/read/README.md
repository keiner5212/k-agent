# read

Read a file. Text files come back as line-numbered text. Supported images (PNG, JPEG, GIF, WebP) are decoded, downscaled so the longest edge fits in 1440 px, and attached for the model. Path is absolute or workspace-relative. Paths outside the workspace wait for the user to allow or deny. Optional offset (1-based) and limit (default 2000 lines) apply to text only and force the text path even on images.

## Does

- Reads a regular file and chooses the path by content and the selected model's input. PNG / JPEG / GIF / WebP are attached only when the model accepts `image`. A PDF is attached only when the model accepts `pdf`. A `.docx` is extracted to text only when the model accepts `document`. Other text stays line-numbered text.
- Text: `BufReader` line scan, 1-based line numbers, optional `offset` (default `1`) and `limit` (default `2000`).
- Text: caps response at `50 KB` of rendered text or 2000 lines, whichever first. Emits a `(Output capped at 50 KB ...)`, `(Showing lines A-B of N ...)`, or `(End of file - total N lines)` footer.
- Text: caps any single line at 2000 chars; longer lines get a `... (line truncated to 2000 chars)` suffix.
- Text: trims trailing `\r\n` / `\n` per line so the footer and line numbers stay aligned.
- Image: reads up to `20 MB`, decodes via the `image` crate (PNG / JPEG / GIF / WebP), encodes back as PNG, and downscales with `thumbnail()` so the longest edge is `<= 1440 px` (no upscale).
- Image: returns `image_png` on the outcome and base64 on `display.image_data`. The chat loop feeds both to OpenAI, Anthropic, and Gemini as a multimodal `tool_result` part.
- Resolves fuzzy sibling suggestions (`Did you mean ...`) when the requested file is missing.
- Returns `startLine` and `endLine` for text, plus `originalWidth` / `originalHeight` for images, so the chat UI can render the editor gutter or the thumbnail correctly.

## Does not

- Edit files. Use `edit` for in-place changes or `write` for full replacements.
- Accept PDF. PDF bytes fail the magic-byte check and the extension list, then surface as `Cannot read binary file`. Use a dedicated PDF tool.
- Accept SVG, HEIC, AVIF, BMP, TIFF. Only PNG / JPEG / GIF / WebP. SVG is XML; HEIC / AVIF / TIFF need separate decoders and are out of scope.
- Accept disguised binary files. Known binary extensions (`.zip`, `.exe`, `.so`, `.wasm`, ...) error out before any byte is read, even if the file also happens to start with PNG magic.
- Read directories. Use `list_directory` instead.
- Follow symlinks that resolve outside the workspace without an allow answer. Outside paths wait for the user.
- Cache file contents. Every call re-reads.
- Downscale below the original size when the source is already smaller than 1440 px (no upscaling).

## Options

| Name       | Type    | Required | Default | Notes                                                              |
| ---------- | ------- | -------- | ------- | ------------------------------------------------------------------ |
| `filePath` | string  | yes      | -       | Absolute or workspace-relative path. Must point to a regular file. |
| `offset`   | integer | no       | `1`     | 1-based start line. Text only. Minimum `1`. Out-of-range errors.   |
| `limit`    | integer | no       | `2000`  | Maximum lines to return. Text only. Minimum `1`.                   |

Setting `offset` or `limit` forces the text path even for images (the model should not ask for line windows on a PNG).

## Response

Text path returns a TOON block with `path`, `startLine`, `endLine`, and `content` (line-numbered body plus footer).

Image path returns a flat TOON record with `path`, `mime`, `bytes`, `width`, `height`, `originalWidth`, `originalHeight`, and `image: png attached`. The PNG bytes travel alongside as `image_png` and `display.image_data` (base64).

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `read tool requires a string \`filePath\`.`: argument missing or wrong type.
- `read tool \`filePath\` is empty.`: argument is whitespace.
- `read tool \`offset\` must be a non-negative integer.`/`at least 1.`/`out of range.`: invalid offset.
- `File not found: <resolved path>` (optionally followed by `Did you mean one of these? ...`).
- `Path is a directory. Use list_directory.`
- `Cannot read binary file: <resolved path>` (extension-based rejection or invalid UTF-8 bytes).
- `Image exceeds 20 MB ingestion limit (<bytes> bytes): <path>`.
- `Image could not be decoded as <PNG|JPEG|GIF|WebP> (<error>): <path>`.
- `Offset <n> is out of range for this file (<m> lines).`
- `Unable to read \`<path>\`: <io error>`.

## Source

`src-tauri/src/tools/read.rs` - entry points: `ReadTool::execute()` for sync, `execute_async()` for the async dispatch path. Magic-byte detection lives in `detect_image_from_bytes()`. Image decode + downscale + PNG re-encode lives in `render_image()`.
