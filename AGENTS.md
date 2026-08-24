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
| Boolean       | `Toggle`                                   |
| Button        | `GlassButton` / `IconButton`               |
| Desktop check | `isTauri()` from `src/lib/platform.ts`     |
| Settings      | `useSettingsStore` + `SETTINGS_REGISTRY`   |
| Providers     | `useProvidersStore`                        |
| Copy          | `t("...")`. No hardcoded UI English.       |
| Token amounts | `formatContextWindow` / `parseTokenAmount` |
| Logical CPUs  | `hardwareThreadCount()`                    |

## Settings

Persist only in `src/lib/settings.ts` (plugin-store file `settings.json`, key `settings`). Sanitize on hydrate. Persist the full `Settings` object.

Add a setting in this order:

1. Field + default on `Settings` / `DEFAULT_SETTINGS` in `src/types/settings.ts`.
2. Sanitize in `sanitizeSettings`.
3. Setter: `set` + `persist(snapshot(get()))`. Put DOM side effects in the setter.
4. Registry item in `registry-data.ts` (`id` = field name).
5. Wire it in `SettingItem`.
6. `en.json` and `es.json`.

Theme, translucency, animations, and text scale go through `applyChrome`. Hydrate, hydrate-fail, and reset all call it. Do not copy those four applies again.

`maxWorkerCores`: `0` = auto (all logical CPUs). Else clamp `1..=hardwareThreadCount()`. Persist only. Do not start thread pools or grep workers yet. Rust `DETAIL_CONCURRENCY` stays a local const until that work reads this setting.

Disable-style toggles (`translucencyEnabled`, `animationsEnabled`): copy says Disable X; `checked` is `!enabled`. Other toggles are positive (`checked === enabled`).

Dynamic select options (core counts) are built in `SettingItem`, not in the static registry.

Window min size is 720x480 in `src/types/settings.ts` and `src-tauri/src/lib.rs`. Keep both in sync.

## Providers and IPC

- Commands return `Result`. Frontend mutations return `{ provider?, error? }` via `runMutation`. Show `error` on the form. Do not warn-and-rethrow.
- `load` without Tauri: empty list. Mutations without Tauri: `{ error: "Desktop shell required" }`.
- Never send API key plaintext back over IPC. Disk uses `enc:v1:` blobs. UI only sees `hasApiKey`. Empty edit keeps the key; explicit clear removes it.
- `ModelInfo` in `src/types/providers.ts` matches the Rust IPC shape, including optional capability fields. The model form does not edit those fields; upsert omits empty vectors so Rust keeps catalog values.
- Provider kind labels: `t("providers.kinds." + kind)`.

## Rust disk layout

All app files live under `~/.k-agent/` (`APP_CONFIG_DIR` in `lib.rs`). Reuse that const.

- `master.key` (mode 0600) via `secret.rs`
- `providers.json`
- `models-dev-cache.json` (24h TTL)

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

- Do not import `models.json` into the webview.
- Do not re-parse the bundled catalog on every command.
- No new global caches, clients, or wrappers. Search first.
- Zustand: select fields, do not subscribe to the whole store in hot views.

## Do not

- Add a second settings store, or a Rust copy of UI settings, until workers need it.
- Put secrets in frontend state, logs, or git.
- Add a second Dialog/Select/Toggle/store/platform helper.
- Comment what the code already says. Comment only a non-obvious why.
- Expand scope past the asked change.
