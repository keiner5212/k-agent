# diagram

Write one Mermaid diagram from a written brief and return the source.

## Does

- Takes `brief`, a description of the picture. Not Mermaid source.
- Calls the app generation model. If that setting is empty, uses the chat model.
- The hidden call gets a Mermaid syntax sheet and returns one fence.
- When `node` is on PATH, runs the packaged `mermaid` checker before returning.
- The checker is `src-tauri/resources/mermaid-check.cjs`. It uses happy-dom. No window opens.
- A parse error is sent back to that model. It retries up to 3 times.
- A source that still fails is not returned. The error is `diagram did not parse:`.
- Without `node`, or without the checker file, the first draft is returned with `checked: false`.
- Rebuild the checker with `pnpm bundle:mermaid-check` after a Mermaid upgrade.
- Output is capped at 4096 tokens.
- The brief is capped at 8000 characters.
- The checker process is killed after 20 seconds.
- `checked` is the string `true` or `false`, not a boolean.

## Does not

- Draw the diagram in the chat. The fence stays source until Visualize.
- Load Mermaid in the chat thread. The viewer imports it when the dialog opens.
- Write Mermaid for the caller. The caller pastes `source` unchanged.
- Use `@mermaid-js/parser`. Intentional. That package has no flowchart grammar.
- Open a browser. The check is a Node process.

## Options

| Name    | Type   | Required | Default | Notes                                     |
| ------- | ------ | -------- | ------- | ----------------------------------------- |
| `brief` | string | yes      | -       | Actors, steps, and links. Max 8000 chars. |

## Response

| Field      | Type   | Notes                                      |
| ---------- | ------ | ------------------------------------------ |
| `status`   | string | `ok` when a source is returned.            |
| `checked`  | string | `true` when Node accepted the source.      |
| `attempts` | string | How many drafts were written.              |
| `source`   | string | Mermaid text. Paste it in a mermaid fence. |

See `response.toon`. The example run has no desktop shell, so it returns an error instead.

## Errors

- `diagram must run on the async path.`
- `diagram requires a brief.`
- `diagram brief exceeds 8000 chars.`
- `diagram needs the desktop shell.`
- `diagram needs a model.`
- `diagram did not parse: <parser message>`
- `node failed to start: <io error>`
- `mermaid parse timed out`
- `mermaid checker failed`

## Source

`src-tauri/src/tools/diagram.rs`, `execute_async()`. The model call is `chat::complete_quiet()`.
