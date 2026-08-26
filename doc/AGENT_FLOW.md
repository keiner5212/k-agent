# Agent flow

How k-agent builds the model system prompt and how the agent should behave on each turn.

## System prompt order

When a chat message is sent, the backend receives one system string assembled on the frontend:

1. **Language rule** (optional). When `forceResponseLanguage` is on, the language directive is first. It overrides conflicting language instructions but not behavior.
2. **Global rules** from `~/.k-agent/AGENTS.md` when that file exists.
3. **Workspace rules** from `{workspace}/AGENTS.md` when that file exists.
4. **Agent system** from `composeAgentSystem()`:
   - **1. Agent flow** - numbered turn protocol.
   - **2. Agent skill loading** - turn-1 batch `skill` calls when the agent has bound global skills not already in context.
   - **3. Agent skills** - names and descriptions only (global skills bound to the agent).
   - **4. Local tools** - tools enabled on that agent (`skill`, `read`, `write`, `edit`, `list_directory`).
   - **5. Workspace skills** - names and descriptions for workspace-local skills.
   - **6. Personality** - agent persona markdown body only.
5. **App context** (optional). Extra app-level notes when configured.

Tool JSON schemas are sent on the request `tools` field, not repeated in the system prompt. Enabled MCP tools are merged into that list at send time (`mcp_{server}_{tool}`).

Skill bodies arrive through tool results after a `skill` call. Persisted on the assistant message (`toolCalls` with `output`) in `sessions.json` and replayed on the next send.

## Turn protocol (model behavior)

The model is instructed to follow this sequence:

| Phase  | Action                                                                               |
| ------ | ------------------------------------------------------------------------------------ |
| Turn 1 | If the agent has bound skills, one batch of `skill` calls for those names. No prose. |
| Next   | Load matching workspace skills with `skill` when they are listed and relevant.       |
| Tools  | Call only advertised tools (local + MCP). Do not invent tool names.                  |
| Answer | Reply using agent personality.                                                       |

Turn 1 batch list omits skills already loaded in the session. Section 2 is omitted when every bound skill is already in context.

## Agent skills vs workspace skills

- **Agent skills** are global skills selected on the agent. Rust `sanitize_skills` only keeps `kind: global`. UI skill picker shows global skills only.
- **Workspace skills** live under `{workspace}/.agents/skills/`. Listed in the system prompt for discovery; loaded on demand via `skill`.
- Session agents are global (`~/.k-agent/agents/`) or builtin. There are no workspace agents.

## Tools

Tools are implemented in `src-tauri/src/tools/`. Each tool is one module; `tools/mod.rs` registers specs and dispatches execution. Paths go through `pathutil` (OS separators, `~`, Windows drive prefixes).

The chat request includes tools enabled on the selected agent plus enabled MCP tools. Tool names that are not registered or not enabled are dropped. A model call to a disabled tool returns an error string instead of running.

Local tools: `skill`, `read`, `write`, `edit`, `list_directory`. MCP tools are invoked via `tools/call` on stdio or HTTP servers.

### Chat tool loop

`send_chat_message` sends `toolNames` from the selected agent and loads MCP tools on the backend. `send_message` in `chat.rs`:

1. Calls the provider with those tool definitions when the list is not empty.
2. If the model returns tool calls, emits a `tool` chunk, then executes each allowed name (local or MCP).
3. Truncates oversized tool results before they go back to the model.
4. Appends assistant tool-call turns and user/tool-result turns to the in-memory turn list.
5. Re-requests until the model returns text only or `MAX_TOOL_ROUNDS` (12) is hit.
6. Streaming is disabled during tool rounds; final text is emitted as chunks after the loop. Assistant text is truncated if it exceeds the output guard.

Gemini function calls echo `thoughtSignature` on later rounds (required by Gemini 3 thinking + tools).

Provider message shapes:

- OpenAI-compatible: `tool_calls` on assistant, `role: tool` results.
- Anthropic: `tool_use` / `tool_result` content blocks. Thinking blocks are echoed on later rounds.
- Gemini: `functionCall` / `functionResponse` parts, plus thought signatures when present.

## Token accounting

- **Personality** alone drives `estimatedTokens` on `AgentMeta` in Rust and builtin agents in the UI.
- **Context usage** is 0 until the first message in the session. After that it splits language, AGENTS.md rules, agent system, tool JSON schemas, MCP tools, persisted skill tool outputs, and conversation.

## Related files

| Area                  | Path                           |
| --------------------- | ------------------------------ |
| System prompt builder | `src/lib/agent-system.ts`      |
| Language + compose    | `src/lib/response-language.ts` |
| Send path             | `src/lib/sessions.ts`          |
| Path sanitization     | `src-tauri/src/pathutil.rs`    |
| Tool registry         | `src-tauri/src/tools/mod.rs`   |
| MCP client            | `src-tauri/src/mcp_client.rs`  |
| Chat + tool loop      | `src-tauri/src/chat.rs`        |
| Agent skill sanitize  | `src-tauri/src/agents.rs`      |
