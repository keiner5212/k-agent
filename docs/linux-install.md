# Linux install

Run this before installing a release build on Linux.

## Symptom

The app is in the system menu. Opening it says the frontend server is not running.

That message comes from a development binary. `pnpm tauri dev` loads the Vite server at `http://localhost:1420`. The menu entry keeps pointing at that binary after the dev server stops. A release build embeds the frontend and does not need the server.

## Before you install

From the repo root:

```sh
sh scripts/linux-prepare-install.sh
```

The script is Linux only. It removes `~/.local/share/applications` entries for k-agent whose program lives under `target/debug` or `target/release`, then refreshes the desktop database.

It also warns when a menu binary is not stripped. That file is a debug build. Do not copy `src-tauri/target/debug/k-agent` to `/usr/bin/k-agent`.

## Install the release

```sh
pnpm tauri build
```

Install the bundle from `src-tauri/target/release/bundle/`. Start k-agent from the menu only after that install. Leave `pnpm tauri dev` for development, and stop it before you use the installed app.
