# k-agent agent conventions

Desktop app: Tauri 2 + Rust + React 19 + TypeScript. One pattern per category. Reuse existing code. Do not invent a second way.

## Inspiration (OpenCode + Command Code)

Product behavior takes cues from [OpenCode](https://github.com/anomalyco/opencode) and [Command Code](https://commandcode.ai/). Both trees live under `.tmp/` (gitignored). Keep them installed there; do not import them into the app.

Expected paths:

- `.tmp/opencode` - OpenCode source
- `.tmp/command-code` - Command Code (`npm i -g command-code` is not enough; clone or unpack into `.tmp/command-code`)

When a feature could follow either product:

1. Ask the user first. Do not pick a design, copy a flow, or start coding from those trees until they confirm.
2. After yes: read both implementations. Compare parse, UI, and data shape.
3. Propose a k-agent middle path that learns from both, then implement only that. Do not clone either UI or paste their code.

Never vendor `.tmp` into `src/` or `src-tauri/`.

## Layout

```
src/components/     reusable UI (Dialog, Select, Toggle, GlassButton, IconButton)
src/features/       product surfaces (settings, providers)
src/lib/            stores, platform, IPC helpers
src/types/          shared TS types and defaults
src/i18n/locales/   en.json + es.json only
src-tauri/src/      Tauri commands and domain
src-tauri/catalog/  bundled models.json (Rust only; never import in the UI)
```

- Components: PascalCase files.
- lib: kebab-case files (`window-bounds.ts`, `use-global-keybindings.ts`).
- Feature UI: PascalCase. Feature data: `registry.ts` + `registry-data.ts`.

## Naming

- TS values/functions: camelCase. Types: PascalCase. Constants: SCREAMING_SNAKE.
- Settings field, zustand setter, persist key, and registry `id` share one camelCase name (`maxWorkerCores`, `translucencyEnabled`).
- Registry copy keys: `titleKey`, `descriptionKey`, `labelKey`.
- IPC JSON is camelCase. Rust fields are snake_case with `#[serde(rename_all = "camelCase")]`.
- i18n: `settings.<field>.label` / `.description` / `.options.*`.
- Commands: snake_case (`list_providers`, `save_provider`).

## Reuse these. Do not add parallels.

| Need          | Use                                        |
| ------------- | ------------------------------------------ |
| Modal         | `Dialog`                                   |
| Dropdown      | `Select` with `{ value, label }`           |
| Data table    | `Table` with column defs + `rowKey`        |
| Boolean       | `Toggle`                                   |
| Button        | `GlassButton` / `IconButton`               |
| Desktop check | `isTauri()` from `src/lib/platform.ts`     |
| Settings      | `useSettingsStore` + `SETTINGS_REGISTRY`   |
| Providers     | `useProvidersStore`                        |
| MCP servers   | `useMcpServersStore`                       |
| Copy          | `t("...")`. No hardcoded UI English.       |
| Token amounts | `formatContextWindow` / `parseTokenAmount` |
| Logical CPUs  | `hardwareThreadCount()`                    |
| Heavy jobs    | `runJob` in `src/lib/jobs.ts`              |

## Settings

Persist only in `src/lib/settings.ts` (plugin-store file `settings.json` in the app data dir, key `settings`). Sanitize on hydrate. Persist the full `Settings` object.

Add a setting in this order:

1. Field + default on `Settings` / `DEFAULT_SETTINGS` in `src/types/settings.ts`.
2. Sanitize in `sanitizeSettings`.
3. Setter: `set` + `persist(snapshot(get()))`. Put DOM side effects in the setter.
4. Registry item in `registry-data.ts` (`id` = field name).
5. Wire it in `SettingItem`.
6. `en.json` and `es.json`.

Theme, translucency, animations, and text scale go through `applyChrome`. Hydrate, hydrate-fail, and reset all call it. Do not copy those four applies again.

`maxWorkerCores`: `0` = auto (all logical CPUs). Else clamp `1..=hardwareThreadCount()`. Sync into `src/lib/worker-cores.ts` on hydrate and on change. Heavy work leases cores with `acquireWorkerCores(label, want)` and always `release()`. Never exceed the configured limit. Never wait for cores (UI must not stall). Grant `min(want, available, limit)` with a floor of 1 so IO still proceeds. Use the full remaining pool when the job is actually parallel (model detail fetch, bundled workspace config list). Shallow IO (one directory, one font scan) leases 1.

Disable-style toggles (`translucencyEnabled`, `animationsEnabled`): copy says Disable X; `checked` is `!enabled`. Other toggles are positive (`checked === enabled`).

Dynamic select options (core counts) are built in `SettingItem`, not in the static registry.

Window min size is 720x480 in `src/types/settings.ts` and `src-tauri/src/lib.rs`. Keep both in sync.

## Providers and IPC

- Commands return `Result`. Frontend mutations return `{ provider?, error? }` via `runMutation`. Show `error` on the form. Do not warn-and-rethrow.
- `load` without Tauri: empty list. Mutations without Tauri: `{ error: "Desktop shell required" }`.
- Never send API key plaintext back over IPC. Disk uses `enc:v1:` blobs in the app data dir. UI only sees `hasApiKey`. Empty edit keeps the key; explicit clear removes it.
- `ModelInfo` in `src/types/providers.ts` matches the Rust IPC shape, including optional capability fields. The model form does not edit those fields; upsert omits empty vectors so Rust keeps catalog values.
- Provider kind labels: `t("providers.kinds." + kind)`.

## Rust disk layout

Secrets and UI settings live in the Tauri app data dir (`app.path().app_data_dir()`). Model and context files live under `~/.k-agent/` (`APP_CONFIG_DIR`). Both locations are built in `src-tauri/src/paths.rs` (`config_dir`, `config_file`, `app_data_dir`, `app_data_file`, `tool_cache_dir`). Do not assemble those paths again. Workspace files live under `{workspace}/.agents/` (`WORKSPACE_AGENTS_DIR`).

App data dir:

- `settings.json` (plugin-store; keys `settings`, `selectedModel`, `selectedAgent`, `modelEffort:*`, `modelRequest:*`)
- `master.key` (mode 0600) via `secret.rs`
- `provider-keys.json` (`enc:v1:` blobs keyed by provider id, mode 0600)
- `mcp-secrets.json` (`enc:v1:` blobs keyed by MCP server id, mode 0600)
- `cache/fetch-url/` and `cache/internet-search/` (tool HTTP cache, 1 hour TTL)
- `sessions.json` (thin index: `activeSessionId` plus `{id,title,preview,updatedAt}`)
- `sessions/{id}/session.json` (messages; no attachment blobs; `toolRounds` only)
- `sessions/{id}/attachments/{attachmentId}{ext}` (raw attachment bytes)
- `sessions/{id}/files/{callId}.before` and `{callId}.after` (write/edit snapshots)

`~/.k-agent/`:

- `providers.json` (no API keys)
- `mcp-servers.json` (no env vars or headers)
- `models-dev-cache.json` (24h TTL)
- `skills/` global skills. Created on first global skill create.
- `agents/` session agents (OpenCode-style personas: `{name}/persona.md`). Not AGENTS.md. Created on first global agent create.
- `AGENTS.md` optional global instruction file. List never creates it. Create/edit/delete from Settings.

Workspace `{workspace}/.agents/skills/`: local skills. Created on first local skill create.
Session agents live in `~/.k-agent/agents/` only. Builtin agents are app-defined.
Workspace `{workspace}/AGENTS.md`: optional workspace instruction file (fallback `agents.md`). List never creates it. Create/edit/delete from Settings. Session agents are not stored as AGENTS.md.

Bundled catalog: `include_str` + parse once (`OnceLock`). Remote overlay, then bundled overlay. User-edited / custom models are not overwritten.

## Session schema migration

Session files under `app_data_dir/sessions/<id>/session.json` and the thin index `sessions.json` are persistent on disk. A schema change to `SessionRecord` or any nested type must not break existing user sessions.

- **Adding a field**: mark it `#[serde(default)]` on the Rust struct and make it `Option<T>` or `Vec<T>` in the TS type. Existing files parse fine; the field defaults to its empty value.
- **Removing or renaming a field**: keep the old name as `#[serde(rename = "...", alias = "...")]` if possible, or write a migration that rewrites old files on first load.
- **Changing a field's shape** (e.g., `priority: enum` to `priority: u8`): do not break deserialization. Add a parallel `legacy_*` parser or a `#[serde(deserialize_with = "...")]` adapter. `sanitizeSessionRecord` in `src/lib/session-turns.ts` is the canonical place to map old shapes to new ones before data reaches the UI.
- **Renaming types or breaking the wire shape**: bump a version field on `SessionRecord`. On load, check the version and migrate or refuse with a clear error.
- **Adding a new sibling field to the active state** (e.g., `todos_history` next to `todos`): `#[serde(default)]` on the new field, plus a default in the two places that construct a fresh `SessionRecord` (`empty_snapshot` and `read_session_record` in `src-tauri/src/sessions.rs`). The frontend sanitizer must also default the new field.

For `sessions.json` (the thin index): same rules, but the index is regenerated from the per-session files on load (`load_from_session_dirs`), so a corrupt index is recoverable by deleting it.

For `~/.k-agent/providers.json` and `mcp-servers.json`: same rules. User-edited, no canonical schema version yet. A change must handle both old and new shapes in one pass.

When in doubt, prefer additive changes (`#[serde(default)]` + new optional fields) over migrations. Less code, less risk.

## Tools

Every tool the LLM can call is one Rust file under `src-tauri/src/tools/`. Wire output is TOON via `toon_doc` (`src-tauri/src/tools/tool-utils/toon.rs`), delegated to the `toon-format` crate. The LLM sees TOON, never YAML. Helpers that are not tools live in `src-tauri/src/tools/tool-utils/`.

Each tool has a dedicated folder under `docs/example/tool-stats/<tool_name>/` with four artifacts:

- `README.md` - hand-written. The specification of the tool.
- `stats.md` - auto-generated. Performance stats from `cargo test --test tools_examples`.
- `response.toon` - auto-generated. A real tool response in the same TOON the LLM sees on the wire.
- `input.json` - auto-generated. The exact JSON arguments the integration test passed to `execute()` for this run, pretty-printed.

`stats.md`, `response.toon`, and `input.json` are regenerated on every `cargo test --test tools_examples`. They are excluded from prettier in `.prettierignore` because the table widths come from Rust `format!`, not from prettier's print width. The hand-written `README.md` IS prettier-checked.

### `README.md` template

Required sections, in this order, exact headings:

1. `# <tool_name>` - H1 with the registered tool name.
2. One-sentence purpose, plain English.
3. `## Does` - concrete bulleted features. Quote real limits (`50 KB`, `2000 lines`, `min(configured, host_cpus, 16)`).
4. `## Does not` - intentional non-goals. Each bullet redirects to the right tool with `Use <other-tool> to ...` or states "Intentional.".
5. `## Options` - markdown table `Name | Type | Required | Default | Notes`. Pulled straight from the JSON schema in `spec()`. Nested shapes (`ask_user.options`) get a sub-table.
6. `## Response` - table of the TOON fields this tool emits. Cross-reference `response.toon`.
7. `## Errors` - one line per error with the literal message prefix that the tool emits.
8. `## Source` - one line: `src-tauri/src/tools/<tool>.rs`, the entry function (`spec()` / `execute()` / `execute_async()` / `replace()`).

Hard constraints:

- ASCII punctuation only. Plain `-`, `:`, `|`, `...`. No smart quotes, em/en dashes, typographic arrows, or emoji.
- Reference tone. No "you", "we", "the user". State facts.
- 60-90 lines per README.

### Tool execution model

Three flavors. Pick by what the tool does, not by what feels easy.

| Flavor                   | When                                                                       | Body                                                                        | Output                                  |
| ------------------------ | -------------------------------------------------------------------------- | --------------------------------------------------------------------------- | --------------------------------------- |
| Sync                     | Pure CPU or memory work, finishes in ms                                    | `Tool::execute` returns `ToolOutcome`                                       | TOON in `text`                          |
| Sync with workspace gate | Reads or writes files anywhere on disk                                     | `Tool::execute` returns `ToolOutcome` after `tool-context!` workspace check | TOON in `text`, may block on `ask_user` |
| Async                    | Waits on user (`ask_user`) or external network (`http_request`, `graphql`) | `Tool::execute_async` future                                                | TOON in `text`, returned via chunk      |

Atomicity rule: every tool is atomic. If the call contains multiple operations (add/update/remove, multi-file write, batched HTTP), build the result in memory first, validate the whole result, then commit. A failed validation in the middle of a call must leave zero side effects. Reference: `TodoTool::stage` in `src-tauri/src/tools/todo.rs` stages all changes in a `Staged` struct before any mutation or persistence.

Chunk emission: tools that produce state the user can see between turns emit a `ChatChunk` through `ctx.on_chunk`. The frontend dispatches by `kind`. Registered kinds: `content`, `reasoning`, `tool`, `question`, `todo`. A new kind needs:

- A `kind` string constant in the Rust tool.
- A `parseXxxChunk` function in `src/lib/sessions.ts` that validates the JSON shape and returns the typed payload, or `null` to drop the chunk.
- A `ChatChunkKind` literal in `src/types/chat.ts`.
- A handler branch in the onmessage callback that updates the relevant store and persists via `persistSnapshot`.

Emit a chunk when the user can scan the result in context (todo list, question options, image preview). Skip the chunk when the user only sees the result after the full call completes (file write, fetch response). A TOON-only tool still feels responsive because the assistant message finalizes on tool completion.

### When adding a new tool

The README template below covers docs and the `cargo test --test tools_examples` block covers the integration test. Plumbing order:

1. Decide the flavor (sync, sync with workspace gate, async). See Tool execution model above.
2. Implement under `src-tauri/src/tools/<name>.rs`. Export `pub const NAME: &str = "<name>";`.
3. Add to `all_tools()` in `mod.rs` and export `pub const <NAME>_TOOL_NAME: &str = ...;` next to the others.
4. Add the tool id to `AGENT_TOOL_IDS` in `src/types/agents.ts`. Decide which agents get it (build, plan, both) and add to `PLAN_AGENT_TOOL_IDS` if relevant.
5. Add `agents.tools.<id>.label` and `agents.tools.<id>.description` in `en.json` and `es.json`. Add `chat.tools.<id>Title` if the tool gets a preview in `ToolCallsBlock`.
6. Add `CHAT_TOOL_DESCRIPTIONS[id]` in `src/types/agents.ts` (English, sent to the LLM).
7. If the result renders inline (like todowrite's `TodoList`), add a renderer branch to `ToolCallsBlock` and a small CSS section in `src/styles/chat.css`. If it opens a modal (like read/write), reuse `ReadOnlyEditorDialog`.
8. Add an integration test block to `src-tauri/tests/tools_examples.rs` driving the tool against a temp workspace and wiring the right `Target` fields.
9. Create `docs/example/tool-stats/<name>/README.md` using the template below. No copy-paste from sibling READMEs - each tool's limits and non-goals are tool-specific.
10. Run `cargo test --test tools_examples` to regenerate `stats.md` and `response.toon` for the new tool.
11. Run all four CI checks before commit.

## i18n

- Language list: `SUPPORTED_LANGUAGES` in `src/types/settings.ts` only.
- Every user-visible string in `en.json` and `es.json`. ASCII in authored JSON.
- Interpolation: `{{name}}`.
- Every entry in `AGENT_TOOL_IDS` (`src/types/agents.ts`) needs `agents.tools.<id>.label` and `agents.tools.<id>.description` in both locales. The agent form reads them via `t(\`agents.tools.${tool}.label\`)` and `t(\`agents.tools.${tool}.description\`)`; missing keys render the raw path as the toggle label. When `ToolCallsBlock` (`src/features/chat/ToolCallsBlock.tsx`) shows a preview for the tool, also add a `chat.tools.<idTitle>Title`entry in both locales.`CHAT_TOOL_DESCRIPTIONS` stays English: it is sent to the LLM, not rendered.

## Errors and edges

- Settings persist failure: `console.warn`, keep memory state.
- Window IPC failure: `console.warn`, do not crash chrome.
- Bad persisted JSON: sanitizer falls back to defaults / clamps.
- Stored `maxWorkerCores` above this machine: clamp on hydrate.
- Prefer `Result` / `{ error }` over panics or empty catch.

## Performance

Goal: the UI never blocks and never feels laggy. Optimize only when the win is real.

Priority order:

1. Keep input and pointer handlers on the fast path. No await, no sync IPC, no layout thrash in the typing path.
2. Cache before refetch. Directory listings: 1s TTL, stale-while-revalidate, show stale data, refresh in the background, no loading flash. Workspace config (skills, agents, AGENTS.md): one bundled job, inflight dedupe, 1s TTL.
3. Cut draw cost. Select only the zustand fields a view needs. Skip extra `setState` when the value did not change. Do not wrap every character in a node. `contain` paint isolation on overlays. `Select` window-virtualizes when options exceed the visible window (fonts, language, model pickers). Do not virtualize variable-height rows (agent selector) or tiny lists (chat `@` mentions cap 20).
4. Respect `maxWorkerCores` and spend what is left. Lease from `acquireWorkerCores`. Parallelize only work that scales (HTTP model details, independent list IPC). One-directory walks stay single-core.
5. Skip low-value machinery. No virtual lists for <=20 rows. No worker-thread filters on tiny arrays. No extra wrappers, caches, or schedulers without a measured hitch.

Also:

- Do not import `models.json` into the webview.
- Do not re-parse the bundled catalog on every command.
- No new global caches, clients, or wrappers. Search first. Reuse `worker-cores`, `runJob`, and per-dir workspace file cache.
- Zustand: select fields, do not subscribe to the whole store in hot views.
- Heavy disk/CPU work (skills, agents, fonts, token estimates, workspace files, future jobs): `runJob` in `src/lib/jobs.ts`. Add a `JobName` and a `handleJob` case. Tauri IPC stays on the UI thread; the worker asks for it with `kind: "invoke"`.

## CSS conventions

Spacing and text sizes follow a fixed scale. Do not pick values off the top of your head.

Spacing scale (`--space-0` through `--space-10`, increments of 4px):

- `--space-1` (4px): tight inline gap, list gap between rows.
- `--space-2` (8px): default gap between related elements, list padding.
- `--space-3` (12px): panel padding, gap between sections.
- `--space-4` (16px): outer padding of large surfaces, dialog body padding.
- `--space-5` (20px) and up: only for major sections (chat thread padding, empty-state vertical centering).

Standard gap between consecutive items in a vertical stack: `--space-2`. Between sections: `--space-3`. Block margin (between content blocks in chat messages): `--space-2`. Padding inside cards and panels: `--space-2 var(--space-3)`.

Text sizes (`--text-*`):

- `--text-content`: canonical body text. Use for chat messages, dialog bodies, todo list items, confirm dialog bodies, markdown headings inside messages. Scales with `--text-scale`.
- `--text-sm` (14px): labels, table cells, inline metadata, button labels, dialog titles.
- `--text-xs` (12px): captions, badges, hints, list-item subtitles, timestamps.
- `--text-md` and up: only for hero text and empty-state titles.

If a new component needs a body text size, use `--text-content`. If a label or caption, use `--text-sm` or `--text-xs`. If you reach for a raw `0.95em`, `1.1em`, etc., stop - those are stale values from before the scale was applied.

Tool result rendering in the chat has two layouts:

- **Inline** (under the tool call line, no modal): for state the user scans in context (todo list, question options, image preview). Insert a small renderer under the tool call line in `ToolCallsBlock`.
- **Modal** (opens on click, full editor): for state the user inspects or diffs (file content, large outputs, structured diffs). Reuse `ReadOnlyEditorDialog` from `src/features/chat/ReadOnlyEditorDialog.tsx`.

Pick inline when the result fits in 5-8 lines and the user reads it as part of the flow. Pick modal when the result is long, structured, or benefits from line numbers and search.

## Checks

Run before commit. CI runs the same four. All must exit zero.

```
pnpm typecheck
pnpm test
pnpm lint
pnpm format:check
```

Order: typecheck, then test, then lint, then format. Fix in that order; lint errors after a type fix are usually follow-ons.

Rust tool tests: `cd src-tauri && cargo test --lib`. Not part of the four; run when touching `src-tauri/src/tools/`.

`pnpm format` (prettier --write) is the only acceptable way to resolve format failures. Do not hand-edit whitespace to satisfy prettier.

## Do not

- Add a second settings store, or a Rust copy of UI settings, until workers need it.
- Put secrets in frontend state, logs, or git.
- Add a second Dialog/Select/Toggle/store/platform helper.
- Comment what the code already says. Comment only a non-obvious why.
- Expand scope past the asked change.
- Ship code with `pnpm format:check`, `pnpm lint`, `pnpm test`, or `pnpm typecheck` failing.
- Start dev servers, watchers, daemons, or any long-running foreground process. They block the shell and stay alive between turns. Prohibited.
