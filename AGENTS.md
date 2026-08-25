# k-agent agent conventions

Desktop app: Tauri 2 + Rust + React 19 + TypeScript. One pattern per category. Reuse existing code. Do not invent a second way.

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

Secrets and UI settings live in the Tauri app data dir (`app.path().app_data_dir()`). Model and context files live under `~/.k-agent/` (`APP_CONFIG_DIR`). Workspace files live under `{workspace}/.agents/` (`WORKSPACE_AGENTS_DIR`).

App data dir:

- `settings.json` (plugin-store; keys `settings`, `selectedModel`, `modelEffort:*`)
- `master.key` (mode 0600) via `secret.rs`
- `provider-keys.json` (`enc:v1:` blobs keyed by provider id, mode 0600)

`~/.k-agent/`:

- `providers.json` (no API keys)
- `models-dev-cache.json` (24h TTL)
- `skills/` global skills. Created on first global skill create.
- `agents/` session agents (OpenCode-style personas: `{name}/persona.md`). Not AGENTS.md. Created on first global agent create.
- `AGENTS.md` optional global instruction file. List never creates it. Create/edit/delete from Settings.

Workspace `{workspace}/.agents/skills/`: local skills. Created on first local skill create.
Workspace `{workspace}/.agents/agents/`: local session agents (`persona.md`). Created on first local agent create. Do not scan `.k-agent` inside a workspace.
Workspace `{workspace}/AGENTS.md`: optional workspace instruction file (fallback `agents.md`). List never creates it. Create/edit/delete from Settings. Session agents are not stored as AGENTS.md.

Bundled catalog: `include_str` + parse once (`OnceLock`). Remote overlay, then bundled overlay. User-edited / custom models are not overwritten.

## i18n

- Language list: `SUPPORTED_LANGUAGES` in `src/types/settings.ts` only.
- Every user-visible string in `en.json` and `es.json`. ASCII in authored JSON.
- Interpolation: `{{name}}`.

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
- Perf logs (`perfLog`) only for slow work. Do not log every keystroke.

## Checks

Run before commit. CI runs the same three. All must exit zero.

```
pnpm typecheck
pnpm lint
pnpm format:check
```

Order: typecheck, then lint, then format. Fix in that order; lint errors after a type fix are usually follow-ons.

`pnpm format` (prettier --write) is the only acceptable way to resolve format failures. Do not hand-edit whitespace to satisfy prettier.

## Do not

- Add a second settings store, or a Rust copy of UI settings, until workers need it.
- Put secrets in frontend state, logs, or git.
- Add a second Dialog/Select/Toggle/store/platform helper.
- Comment what the code already says. Comment only a non-obvious why.
- Expand scope past the asked change.
- Ship code with `pnpm format:check`, `pnpm lint`, or `pnpm typecheck` failing.
