# k-agent

Desktop AI agent orchestrator.

Stack: Deno + Hono + WebView + Next.js (static export) in a pnpm monorepo.

## Author

Maintained by Keiner Jose Alvarado Quintero. Contact: <keinerjosealvaradoquintero@gmail.com>

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
Dev mode:

```
pnpm dev
# or
pnpm dev:[web|desktop]
```

Build and sync:

```
pnpm build:[web|desktop] && pnpm sync:[web|desktop]
```

## Contributing

Issues and pull requests are welcome. For larger changes, open an issue first to discuss what you want to change.

## Donations

If this project is useful to you, consider supporting its development:

[Donate via PayPal](https://www.paypal.com/donate/?business=ja8542159@gmail.com)

## License

See the LICENSE file.