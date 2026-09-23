# System instruction

One string, built when a chat message is sent, passed as `system`.

OpenCode personalities use markdown headings, `IMPORTANT` lines, and `<example>` blocks. That mix has no formal spec name. The tag part is XML-tagged prompting (the same layout as the prompt improver). k-agent wraps each source in a tag so those headings stay inside `<personality>`.

## Order

Empty blocks are omitted.

1. `<language>` - only when force-response-language is on. `src/lib/response-language.ts`.
2. `<global-rules>` - text from `~/.k-agent/AGENTS.md`.
3. `<workspace-rules>` - text from `{workspace}/AGENTS.md`.
4. `<agent-flow>` - turn order. `composeAgentSystem()` in `src/lib/agent-system.ts`.
5. `<agent-skills>` - global skills bound to the agent that are not already loaded.
6. `<workspace-skills>` - skills under `{workspace}/.agents/skills/` that are not already loaded.
7. `<clarify>` - call `ask_user` when the request is not fully clear. Omitted when the agent has no `ask_user` tool.
8. `<todos>` - call `todowrite` whenever a step starts, finishes, drops, or changes. Omitted when the agent has no `todowrite` tool.
9. `<personality>` - the agent persona body, unchanged.
10. `<rendering>` - how chat markdown is shown.
11. `<app-context>` - extra app notes. Omitted while that list is empty.

Tool JSON schemas go on the request `tools` field, not in this string. A skill body arrives later, as the result of a `skill` call.

## Turn 1

If `<agent-skills>` is present, the first model turn is one batch of `skill` calls and no prose. After that, load a workspace skill only when the task needs it. Then answer with the personality.

Annotated example: [system-prompt.html](example/system-prompt.html). Plain text of that same string: [system-prompt.md](example/system-prompt.md).
