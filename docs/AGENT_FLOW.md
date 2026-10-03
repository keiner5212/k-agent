# System instruction

One string, built when a chat message is sent, passed as `system`.

OpenCode personalities use markdown headings, `IMPORTANT` lines, and `<example>` blocks. That mix has no formal spec name. The tag part is XML-tagged prompting (the same layout as the prompt improver). k-agent wraps each source in a tag so those headings stay inside `<personality>`.

## Order

Empty blocks are omitted.

1. `<language>` - only when force-response-language is on. `src/lib/response-language.ts`.
2. `<environment>` - workspace, host OS, home, shell, and app config. `src-tauri/src/host_context.rs`. Omitted when the desktop shell does not answer.
3. `<global-rules>` - text from `~/.k-agent/AGENTS.md`.
4. `<workspace-rules>` - text from `{workspace}/AGENTS.md`.
5. `<agent-flow>` - turn order. `composeAgentSystem()` in `src/lib/agent-system.ts`.
6. `<agent-skills>` - global skills bound to the agent that are not already loaded.
7. `<workspace-skills>` - skills under `{workspace}/.agents/skills/` that are not already loaded.
8. `<clarify>` - call `ask_user` when the request is not fully clear. Omitted when the agent has no `ask_user` tool.
9. `<todos>` - call `todowrite` with the full list when a multi-step task starts, finishes, or drops a step. No ids. Omitted when the agent has no `todowrite` tool.
10. `<tools>` - dedicated tools instead of `bash`. Omitted when the agent has no `bash` tool. Lines for missing tools are omitted.
11. `<agents>` - other agents `task` may call. Omitted when `task` is off or no other agent exists.
12. `<personality>` - the agent persona body, unchanged.
13. `<rendering>` - how chat markdown is shown.
14. `<visual-check>` - `page_shot` rules. Omitted when the agent has no `page_shot` tool.
15. `<mcp-tools>` - enabled MCP tools. Omitted when none are enabled.
16. `<workspace-notes>` - `.k-agent/NOTES.md` rules. Omitted when workspace memory is off.
17. `<app-context>` - extra app notes. Omitted while that list is empty.

Tool JSON schemas go on the request `tools` field, not in this string. A skill body arrives later, as the result of a `skill` call.

## Turn 1

If `<agent-skills>` is present, the first model turn is one batch of `skill` calls and no prose. After that, load a workspace skill only when the task needs it. Then answer with the personality.

Annotated example: [system-prompt.html](example/system-prompt.html). Plain text of that same string: [system-prompt.md](example/system-prompt.md).
