# Example: a typical chat turn

This directory contains a 1:1 example of the inputs and outputs of a
typical chat turn: the exact system prompt that the frontend sends to
Rust for the LLM, and the exact tool call / tool result formats that
Rust emits.

This is documentation, not generated. It must be kept in sync with:

- `src/lib/agent-system.ts` (`composeAgentSystem`).
- `src/lib/response-language.ts` (`composeSystemWithLanguage`).
- `src-tauri/src/chat.rs` (`openai_messages`, `emit_chunk`,
  `emit_tool_call`, round loop).
- `src-tauri/src/tools/*.rs` (`toon_doc` outputs).

## Scenario

- Workspace has one agent-bound skill (`code-review`, in
  `~/.k-agent/skills/`) and one workspace skill (`commit`, in
  `.agents/skills/`).
- `~/.k-agent/AGENTS.md` and `<workspace>/AGENTS.md` are both empty.
- `forceResponseLanguage` is off.
- User message: `What does the foo function do?`
- Selected agent: `build` (default built-in).

## Files

- [`system-prompt.md`](./system-prompt.md) - the exact text of the
  `system` message the frontend sends to Rust.
- [`tool-stats/<tool>/`](./tool-stats/) - one folder per tool with its
  `README.md` (hand-written spec), `response.toon` (a real example
  response in the wire format), and `stats.md` (performance metrics).
