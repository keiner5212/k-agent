# k-agent

AI coding model orchestrator. Desktop shell built with Tauri 2 + Rust + React 19 + TypeScript.

## Status

Blank shell. Settings dialog (gear icon, `Mod+P`) with language and keybinding configuration. Ready to be extended with provider orchestration.

## Requirements

- Node.js >= 20.19
- pnpm >= 9 (project pins `pnpm@11.2.2`)
- Rust toolchain >= 1.77.2 with `cargo` (only needed for `pnpm tauri dev` / `pnpm tauri build`)
- Platform deps for Tauri: see <https://v2.tauri.app/start/prerequisites/>

If Rust is not installed, `pnpm dev` still works for browser preview of the frontend.

## Install

```sh
pnpm install
```

## Run

Browser-only frontend (no Tauri shell, no Rust needed):

```sh
pnpm dev
```

Full desktop app (requires Rust):

```sh
pnpm tauri dev
```

## Build

Native bundle:

```sh
pnpm tauri build
```

Frontend only:

```sh
pnpm build
```

## Scripts

| Script              | Purpose                                       |
| ------------------- | --------------------------------------------- |
| `pnpm dev`          | Vite dev server on `http://localhost:1420`    |
| `pnpm build`        | Type-check + Vite production build to `dist/` |
| `pnpm preview`      | Preview the built frontend                    |
| `pnpm typecheck`    | TypeScript only                               |
| `pnpm lint`         | ESLint flat config                            |
| `pnpm lint:fix`     | ESLint with autofix                           |
| `pnpm format`       | Prettier write                                |
| `pnpm format:check` | Prettier check                                |
| `pnpm tauri`        | Pass-through to `@tauri-apps/cli`             |
| `pnpm dev:tauri`    | Full Tauri dev (Rust + frontend)              |
| `pnpm build:tauri`  | Full Tauri release build                      |

## Project layout

```
.
|-- src/                # React 19 frontend (TS)
|   |-- components/     # Generic UI primitives (Dialog, GearButton)
|   |-- features/
|   |   `-- settings/   # Settings dialog + sections
|   |-- i18n/           # i18next config + en/es locales
|   |-- lib/            # keybindings, settings store, platform
|   |-- styles/         # tokens.css + reset + global
|   |-- types/          # Shared TS types
|   |-- App.tsx
|   `-- main.tsx
|-- src-tauri/          # Rust backend (Tauri 2)
|   |-- src/
|   |   |-- main.rs
|   |   `-- lib.rs
|   |-- capabilities/   # Permission grants
|   |-- icons/
|   |-- Cargo.toml
|   |-- build.rs
|   `-- tauri.conf.json
|-- index.html
|-- vite.config.ts
|-- tsconfig*.json
|-- eslint.config.js
`-- .prettierrc.json
```

## Keyboard

| Action         | Default                                        |
| -------------- | ---------------------------------------------- |
| Open settings  | `Mod+P` (`Cmd+P` on macOS, `Ctrl+P` elsewhere) |
| Close settings | `Escape`                                       |

Keybindings are user-rebindable from `Settings > Keybindings`.

## Language

English and Spanish ship by default. Add a new locale by dropping a JSON file in `src/i18n/locales/` and registering it in `src/i18n/index.ts` plus `src/types/settings.ts`.

## Stack

- Tauri `2.11.5`
- `@tauri-apps/api` `2.11.1`
- `@tauri-apps/plugin-store` `2.4.4`
- React `19.2.8`
- Vite `8.2.2`
- TypeScript `7.0.2`
- i18next `26.4.0` + react-i18next `17.0.12`
- @radix-ui/react-dialog `1.1.23`
- lucide-react `1.33.0`
- zustand `5.0.15`
- ESLint `10.9.0` (flat config)
- Prettier `3.9.6`

## License

See `LICENSE`.
