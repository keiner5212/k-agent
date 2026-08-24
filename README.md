# k-agent

Desktop AI coding orchestrator. Tauri 2 + Rust + React 19 + TypeScript.

## Author

Maintained by Keiner Jose Alvarado Quintero. Contact: <keinerjosealvaradoquintero@gmail.com>

## Start

```sh
pnpm install
pnpm dev              # browser preview (no Rust needed)
pnpm tauri dev        # desktop window (requires Rust >= 1.77.2)
```

## Build

```sh
pnpm build            # frontend only -> dist/
pnpm tauri build      # native bundle
```

## Tooling

```sh
pnpm typecheck
pnpm lint
pnpm lint:fix
pnpm format
pnpm format:check
```

## Settings

Open with the gear icon or `Mod+P` (`Cmd+P` / `Ctrl+P`). Sidebar nav + content pane.

- **General**: language, theme, text size, minimize to system tray, translucency, animations
- **Providers**: AI endpoints (OpenAI / Anthropic / Gemini compatible). Models auto-detect on save.
- **Keybindings**: rebind any shortcut inline

## Donations

If this project is useful to you, consider supporting its development:

[Donate via PayPal](https://www.paypal.com/donate/?business=ja8542159@gmail.com)

## License

See `LICENSE`.
