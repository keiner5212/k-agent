# validate_mermaid

Check one mermaid diagram with the same parser the chat uses to draw it. Call it before a mermaid fence goes in the reply.

The reply may include that fence only after this tool returns `status: "ok"`.

## Does

- Parse `source` with `mermaid.parse` from mermaid 12, the same parser that draws chat diagrams.
- Cover every diagram type that parser accepts, including flowchart, sequence, class, state, ER, and gantt.
- Strip one wrapping mermaid fence when `source` includes the fence and the closing ticks.
- Return `status: "ok"` when the diagram parses. No other fields.
- Return `status: "error"` and the parser message when it does not. The message is clipped at `2000` chars.
- Reject an empty source and a source over `100000` chars before the parser starts.
- Run the parser in a local `node` process so the check matches the chat, not a shorter subset of the grammar.
- Allow several calls in one turn. Each call checks one diagram.

## Does not

- Draw the diagram. The chat renders a fenced `mermaid` block after a successful check.
- Repair the diagram. Fix the source and call again.
- Accept prose around the diagram. Pass the diagram source only.
- Run without `node` on `PATH` and the `mermaid` package in this repo's `node_modules`, or in `node_modules` next to the executable.
- Cache a successful parse. Each call parses again.
- Read a file. Pass the diagram text in `source`. Use `read` for files.
- Emit a chat chunk. The diagram appears only when the reply contains a mermaid fence.

## Options

| Name     | Type   | Required | Default | Notes                                                                   |
| -------- | ------ | -------- | ------- | ----------------------------------------------------------------------- |
| `source` | string | yes      | -       | Diagram source. Max 100000 chars. A wrapping mermaid fence is stripped. |

## Response

| Field    | Type   | Notes                                                           |
| -------- | ------ | --------------------------------------------------------------- |
| `status` | string | `ok` when the parser accepts the source. `error` otherwise.     |
| `error`  | string | Parser or setup message. Present only when `status` is `error`. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `validate_mermaid: <serde error>`: argument shape did not match the schema.
- `validate_mermaid: source is empty.`: source was blank after trim, or the fence had no body.
- `validate_mermaid: source exceeds 100000 chars.`: source too long.
- `validate_mermaid: node is not available.`: the `node` binary did not start.
- `validate_mermaid: mermaid package was not found.`: `node_modules/mermaid` was not next to the repo or the executable.
- `validate_mermaid: parser exited without a result.`: node exited before writing a parse result.
- `validate_mermaid: parser task failed`: the background parse task did not finish.
- `validate_mermaid: could not write the diagram.`: the parser process had no stdin.
- `status: error` with `error` set to the mermaid parser text: the diagram is invalid. This is a normal tool result, not a crash.

A call looks like `{"source":"flowchart LR\n  A[Status line] --> B[Headers]"}`. A broken sequence diagram returns `status: "error"` and the parser line, for example `Parse error on line 3`.

The chat system prompt tells the model to call this tool before it writes a mermaid fence, and to include the fence only after `status: "ok"`.

## Source

`src-tauri/src/tools/validate_mermaid.rs` - entry point: `spec()` / `execute_async()`.
