# page_shot

Capture one http or https page in a hidden WebKit view and return a PNG.

## Does

- Loads the URL in a hidden utility window: no decorations, no taskbar entry, on-screen at opacity 0 so WebKit still paints. The page is not shown.
- Viewport default is `1280x720`. Width is clamped to `320-1600`. Height is clamped to `240-1200`. Height is the window, not the document.
- Waits for the load to finish, scrolls a URL hash into view, then waits `waitMs` (default `300`, max `5000`) before capture.
- Rejects a nearly solid black or white frame. That frame is a capture miss, not the page.
- With no `selector`, returns the visible viewport.
- With `selector`, uses the same viewport and crops the PNG to that element's box.
- Returns the PNG to the model as an image part and shows the same image in the chat tool row.
- Accepts localhost and private hosts. This is a frontend check, not a public-page reader.

## Does not

- Run on hosts without Linux WebKit. Other platforms return `page_shot currently requires Linux WebKit.`
- Click or type. Intentional. A URL hash is the only scroll, and it runs before the shot.
- Read page text. Use `fetch_url` for a public article, or `http_request` for a raw response.
- Show a browser window. The capture stays hidden.
- Capture the full document. Use one viewport shot. A hash scrolls that section into the window.
- Start a server. The page must already be listening. Use `background` for a dev server. It stops when the turn ends.

## Options

| Name       | Type    | Required | Default | Notes                                        |
| ---------- | ------- | -------- | ------- | -------------------------------------------- |
| `url`      | string  | yes      | -       | http or https URL, including localhost.      |
| `width`    | integer | no       | `1280`  | Viewport width. Clamped to 320-1600.         |
| `height`   | integer | no       | `720`   | Viewport height. Clamped to 240-1200.        |
| `selector` | string  | no       | -       | CSS selector. When set, the PNG is that box. |
| `waitMs`   | integer | no       | `300`   | Extra wait after load. Clamped to 0-5000.    |

## Response

| Field      | Notes                                               |
| ---------- | --------------------------------------------------- |
| `url`      | URL that was requested.                             |
| `width`    | Viewport width used.                                |
| `height`   | Viewport height used.                               |
| `selector` | Selector, or empty when the whole viewport is shot. |
| `bytes`    | PNG size in bytes.                                  |
| `image`    | `png attached`. The bytes travel beside the text.   |

The chat row renders `display.imageData`. See `response.toon` for the text fields.

## Errors

- `page_shot requires a non-empty `url`.`: missing url.
- `page_shot must run on the async dispatch path.`: sync `execute`.
- `page_shot URL is not parseable.`: bad URL.
- `page_shot only accepts http and https URLs.`: other scheme.
- `page_shot requires the desktop shell.`: no app handle.
- `page_shot currently requires Linux WebKit.`: non-Linux host.
- `page_shot timed out.`: capture exceeded 25 seconds.
- `page_shot selector matched no element.`: selector missed.
- `page_shot selector matched an empty element.`: box has no size.
- `page_shot snapshot: <error>`: WebKit snapshot failed.
- `page_shot captured a blank frame.`: the pixels were nearly one flat black or white. Retry once with a higher `waitMs`.

## Source

`src-tauri/src/tools/page_shot.rs` - `spec()` / `execute()` / `execute_async()`. Linux capture: `capture_linux()` / `shoot()` / `encode_crop()`.
