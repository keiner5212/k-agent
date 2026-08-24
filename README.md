# k-agent

Desktop AI agent orchestrator. Empty template.

Stack: Deno + Hono + WebView.

## Install

Requires [Deno](https://docs.deno.com/runtime/getting_started/installation/) 1.45+.

```
deno install
```

On Linux, the desktop window needs WebKit:

```
sudo apt install libwebkit2gtk-4.1-0
```

## Run

```
deno task dev
```

Open <http://127.0.0.1:7421> in a browser if the window fails to open.

## Build binary

```
deno task compile
./bin/k-agent
```

## Author

Keiner Jose Alvarado Quintero. Contact: <keinerjosealvaradoquintero@gmail.com>

## Contributing

Issues and pull requests are welcome.

## Donations

[Donate via PayPal](https://www.paypal.com/donate/?business=ja8542159@gmail.com)

## License

See LICENSE.