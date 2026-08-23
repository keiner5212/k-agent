# k-agent

Desktop AI agent orchestrator. Empty template.

Stack: Deno + Hono + WebView + Next.js (static export) in a pnpm monorepo.

## Author

Maintained by [Your Name]. Contact: <ja8542159@gmail.com>

## Install

Requires Node 20+, pnpm 9+, and Deno 1.45+.

```
pnpm install
```

If `deno` is missing:

```
# macOS / Linux
curl -fsSL https://deno.land/install.sh | sh

# Windows (PowerShell)
irm https://deno.land/install.ps1 | iex
```

Restart your shell after installing Deno.

## Run

Web dev (browser only):

```
pnpm dev:web
```

Desktop (Hono server, native window when binding is installed):

```
pnpm build:web && pnpm sync:web && pnpm dev:desktop
```

Server runs at <http://127.0.0.1:7421>.

## Contributing

Issues and pull requests are welcome. For larger changes, open an issue first to discuss what you want to change.

## Donations

If this project is useful to you, consider supporting its development:

[Donate via PayPal](https://www.paypal.com/donate/?business=ja8542159@gmail.com)

## License

See the LICENSE file.