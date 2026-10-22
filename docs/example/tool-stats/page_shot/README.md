# page_shot

Capture one http or https page in a hidden WebKit view and return a PNG.

## Does

- Loads the URL in a utility window: no decorations, no taskbar entry, mapped off-screen at full opacity so WebKit paints. The page is not shown.
- Viewport default is `1280x720`. Width is clamped to `320-1600`. Height is clamped to `240-1200`. Height is the window, not the document.
- Ignores `about:` and empty documents. After load, polls until the view has a size and a non-blank paint, up to 10 seconds, inside the 25 second cap. Then waits `waitMs` (default `300`, max `5000`).
- Rejects a nearly solid black or white frame after those waits. That frame is a capture miss, not the page. One call does the waits. The model does not retry with another host or height.
- `localStorage` and `sessionStorage` are `{key, value}` lists. They are set on the page origin, then `url` loads again so the app script sees them. The shot is that load. Each list is at most 50 items. Each value is at most 8 KB.
- `cookies` are set with the WebKit cookie API before the first request, so `httpOnly` is sent. Default domain is the URL host. Default path is `/`. A domain that does not match the host is rejected. Same 50 item and 8 KB caps.
- Over-limit storage or cookies return `page_shot ...` and do not open a window. Counts come back in the response. Values do not.
- With no `selector`, returns the visible viewport. With `selector`, crops the PNG to that element.

## Does not

- Run on hosts without Linux WebKit. Other platforms return `page_shot currently requires Linux WebKit.`
- Click or type. Intentional. A URL hash is the only scroll, and it runs before the shot.
- Set cookies with `document.cookie`. Intentional. `httpOnly` would be dropped.
- Send headers, proxy, or an auth flow. Intentional.
- Read page text. Use `fetch_url` for a public article.
- Start a server. The page must already be listening. Use `background` for a dev server. It stops when the turn ends.

## Options

| Name             | Type    | Required | Default | Notes                                                           |
| ---------------- | ------- | -------- | ------- | --------------------------------------------------------------- |
| `url`            | string  | yes      | -       | http or https URL, including localhost.                         |
| `width`          | integer | no       | `1280`  | Viewport width. Clamped to 320-1600.                            |
| `height`         | integer | no       | `720`   | Viewport height. Clamped to 240-1200.                           |
| `selector`       | string  | no       | -       | CSS selector. When set, the PNG is that box.                    |
| `waitMs`         | integer | no       | `300`   | Extra delay after a non-blank paint. Clamped to 0-5000.         |
| `localStorage`   | array   | no       | -       | Up to 50 `{key, value}` strings. Applied before the app script. |
| `sessionStorage` | array   | no       | -       | Same shape and timing as `localStorage`.                        |
| `cookies`        | array   | no       | -       | Up to 50 cookies, set before the first request.                 |

### `localStorage` and `sessionStorage` item

| Name    | Type   | Required | Default | Notes                           |
| ------- | ------ | -------- | ------- | ------------------------------- |
| `key`   | string | yes      | -       | Storage key. Must be non-empty. |
| `value` | string | yes      | -       | Storage value. Max 8 KB.        |

### `cookies` item

| Name       | Type    | Required | Default  | Notes                                    |
| ---------- | ------- | -------- | -------- | ---------------------------------------- |
| `name`     | string  | yes      | -        | Cookie name. Must be non-empty.          |
| `value`    | string  | yes      | -        | Cookie value. Max 8 KB.                  |
| `domain`   | string  | no       | URL host | Must match the URL host. Leading dot ok. |
| `path`     | string  | no       | `/`      | Cookie path.                             |
| `secure`   | boolean | no       | `false`  | Secure flag.                             |
| `httpOnly` | boolean | no       | `false`  | HttpOnly flag. Set through WebKit.       |

## Response

| Field            | Notes                                               |
| ---------------- | --------------------------------------------------- |
| `url`            | URL that was requested.                             |
| `width`          | Viewport width used.                                |
| `height`         | Viewport height used.                               |
| `selector`       | Selector, or empty when the whole viewport is shot. |
| `localStorage`   | Count of storage entries applied. Not the values.   |
| `sessionStorage` | Count of session entries applied. Not the values.   |
| `cookies`        | Count of cookies applied. Not the values.           |
| `bytes`          | PNG size in bytes.                                  |
| `image`          | `png attached`. The bytes travel beside the text.   |

## Errors

- `page_shot requires a non-empty `url`.`: missing url.
- `page_shot must run on the async dispatch path.`: sync `execute`.
- `page_shot URL is not parseable.`: bad URL.
- `page_shot URL has no host.`: no host to bind storage or cookies.
- `page_shot only accepts http and https URLs.`: other scheme.
- `page_shot localStorage ...`, `page_shot sessionStorage ...`, `page_shot cookies ...`: bad list, type, empty name, or size. No window is opened.
- `page_shot cookies[n] domain does not match the URL host.`: cookie domain is a different host.
- `page_shot requires the desktop shell.`: no app handle.
- `page_shot currently requires Linux WebKit.`: non-Linux host.
- `page_shot timed out.`: capture exceeded 25 seconds.
- `page_shot selector matched no element.`: selector missed.
- `page_shot selector matched an empty element.`: box has no size.
- `page_shot snapshot: <error>`: WebKit snapshot failed.
- `page_shot captured a blank frame. A blank frame is not the page.`: pixels stayed nearly one flat black or white after the internal waits.

## Source

`src-tauri/src/tools/page_shot.rs` - `spec()` / `execute()` / `execute_async()`. Linux capture: `capture_linux()` / `shoot()` / `encode_crop()`.
