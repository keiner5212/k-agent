# lsp

Ask the language server installed for a file. One operation per call.

## Does

- Resolves the server from the file path and the servers the user installed in Settings.
- Opens the document, or sends `didChange` when `write`, `edit`, `delete`, or `apply_patch` already changed it.
- `goToDefinition`, `findReferences`, `hover`, `documentSymbol`, `workspaceSymbol`.
- `goToImplementation`, `prepareCallHierarchy`, `incomingCalls`, `outgoingCalls`.
- `line` and `character` are 1-based. The wire to the server is 0-based.
- `workspaceSymbol` uses `query`. The file path only picks the server.
- Renders locations as `path:line:character`, at most 40. Hover text is capped at 1200 characters.
- Returns `none` when the server has no hit.
- `incomingCalls` and `outgoingCalls` prepare the call item first, then ask for the calls.
- A request times out after 20 seconds.
- Locations use the workspace-relative path when the file is inside the workspace.
- The client advertises definition, references, hover, symbols, and call hierarchy.

## Does not

- Install a server. Use Settings.
- Run when LSP is disabled. The error is `language servers disabled`.
- Guess a definition from search results. Use `grep` for text search.
- Return a pretty-printed JSON dump. The body is a short line list.

## Options

| Name        | Type    | Required | Default | Notes                                              |
| ----------- | ------- | -------- | ------- | -------------------------------------------------- |
| `operation` | string  | yes      | -       | One of the nine operations above.                  |
| `filePath`  | string  | yes      | -       | Absolute or workspace-relative file.               |
| `line`      | integer | no       | 1       | 1-based. Ignored by document and workspace symbol. |
| `character` | integer | no       | 1       | 1-based.                                           |
| `query`     | string  | no       | empty   | `workspaceSymbol` filter.                          |

## Response

| Field       | Type   | Notes                           |
| ----------- | ------ | ------------------------------- |
| `operation` | string | The operation that ran.         |
| `path`      | string | Workspace-relative file.        |
| `result`    | string | Locations, hover text, or none. |

See `response.toon`.

## Errors

- `lsp requires operation.`
- `lsp requires filePath.`
- `lsp needs the desktop shell.`
- `File not found: <path>`
- `language servers disabled`
- `no language server for this file`
- `language server not installed: <id>`
- `lsp must run on the async path.`

## Source

`src-tauri/src/tools/lsp.rs` - `execute_async()`. Document sync is `lsp_client::sync_disk_change()`.
