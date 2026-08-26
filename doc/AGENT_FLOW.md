# Agent flow

How k-agent builds the model system prompt and how the agent should behave on each turn.

## System prompt order

When a chat message is sent, the backend receives one system string assembled on the frontend:

1. **Language rule** (optional). When `forceResponseLanguage` is on, the language directive is first. It overrides conflicting language instructions but not behavior.
2. **Global rules** from `~/.k-agent/AGENTS.md` when that file exists.
3. **Workspace rules** from `{workspace}/AGENTS.md` when that file exists.
4. **Agent system** from `composeAgentSystem()`:
   - **Flow** - turn-1 skill batch when the agent has bound skills; workspace skill load-on-demand; use advertised tools; then personality.
   - **Agent skills** - names and descriptions only (global skills bound to the agent).
   - **Workspace skills** - names and descriptions for workspace-local skills.
   - **Personality** - agent persona markdown body only.
5. **App context** (optional). Extra app-level notes when configured.

Tool JSON schemas are sent on the request `tools` field, not repeated in the system prompt.

Skill bodies arrive through tool results after a `skill` call. Persisted on the assistant message (`toolCalls` with `output`) in `sessions.json` and replayed on the next send.

## Turn protocol (model behavior)

The model is instructed to follow this sequence:

| Phase  | Action                                                                               |
| ------ | ------------------------------------------------------------------------------------ |
| Turn 1 | If the agent has bound skills, one batch of `skill` calls for those names. No prose. |
| Next   | Load matching workspace skills with `skill` when they are listed and relevant.       |
| Tools  | Call only advertised tools. Do not invent tool names.                                |
| Answer | Reply using agent personality.                                                       |

Turn 1 is omitted from the prompt when the agent has no bound skills (e.g. builtin build/plan).

## Agent skills vs workspace skills

- **Agent skills** are global skills selected on the agent. Rust `sanitize_skills` only keeps `kind: global`. UI skill picker shows global skills only.
- **Workspace skills** live under `{workspace}/.agents/skills/`. Listed in the system prompt for discovery; loaded on demand via `skill`.
- Session agents are global (`~/.k-agent/agents/`) or builtin. There are no workspace agents.

## Tools

Tools are implemented in `src-tauri/src/tools/`. Each tool is one module; `tools/mod.rs` registers specs and dispatches execution.

The chat request only includes tools enabled on the selected agent. Tool names that are not registered or not enabled are dropped. A model call to a disabled tool returns an error string instead of running.

### `skill` tool

- **Name:** `skill`
- **Input:** `{ "name": "<skill name>" }`
- **Behavior:** Finds a global or workspace skill by name, returns the SKILL.md body and `dir: <path>`.
- **Registration:** Add new tools beside `skill.rs` and register in `all_tools()` inside `mod.rs`.

### Chat tool loop

`send_chat_message` sends `toolNames` from the selected agent. `send_message` in `chat.rs`:

1. Calls the provider with those tool definitions when the list is not empty.
2. If the model returns tool calls, emits a `tool` chunk with the tool name (and skill name for `skill`), then executes each allowed name via `tools::execute`.
3. Appends assistant tool-call turns and user/tool-result turns to the in-memory turn list.
4. Re-requests until the model returns text only or `MAX_TOOL_ROUNDS` (12) is hit.
5. Streaming is disabled during tool rounds; final text is emitted as chunks after the loop.

Provider message shapes:

- OpenAI-compatible: `tool_calls` on assistant, `role: tool` results.
- Anthropic: `tool_use` / `tool_result` content blocks. Thinking blocks are echoed on later rounds.
- Gemini: `functionCall` / `functionResponse` parts.

## Token accounting

- **Personality** alone drives `estimatedTokens` on `AgentMeta` in Rust and builtin agents in the UI.
- **Context usage** splits language, AGENTS.md rules, agent system, tool JSON schemas, persisted skill tool outputs, and conversation (content, reasoning, attachment text).

## Related files

| Area                  | Path                           |
| --------------------- | ------------------------------ |
| System prompt builder | `src/lib/agent-system.ts`      |
| Language + compose    | `src/lib/response-language.ts` |
| Send path             | `src/lib/sessions.ts`          |
| Tool registry         | `src-tauri/src/tools/mod.rs`   |
| Skill tool            | `src-tauri/src/tools/skill.rs` |
| Chat + tool loop      | `src-tauri/src/chat.rs`        |
| Agent skill sanitize  | `src-tauri/src/agents.rs`      |
