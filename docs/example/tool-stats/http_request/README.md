# http_request

Send one HTTP request and return the status, headers, and body.

## Does

- Accepts any `http` or `https` URL, including localhost and private hosts.
- Accepts methods `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`, and `OPTIONS`. Default `GET`.
- Appends a `query` object to the URL and sends a `headers` object as request headers.
- Sends `body` for methods other than `GET` and `HEAD`. Body cap is `1 MB`.
- Follows at most 5 redirects. Timeout default `20000` ms, range `1000-60000`.
- Truncates the response body at `200000` bytes and sets `truncated` to `1`.
- Runs `GET` and `HEAD` immediately. Any other method waits on Deny, Accept this time, or Accept for this chat.
- Accept for this chat stores `httpWriteAllowed` on the session. It does not grant filesystem access.

## Does not

- Read a page as an article. Use `fetch_url` to read a public page.
- Search the web. Use `internet_search` to find URLs.
- Capture pixels. Use `page_shot` to screenshot a page.
- Send credentials from the app secret store. Intentional. Pass headers explicitly.

## Options

| Name        | Type    | Required | Default | Notes                                       |
| ----------- | ------- | -------- | ------- | ------------------------------------------- |
| `url`       | string  | yes      | -       | Absolute http or https URL. Max 4096 chars. |
| `method`    | string  | no       | `GET`   | One of the methods listed above.            |
| `headers`   | object  | no       | -       | String values. Non-strings are JSON text.   |
| `query`     | object  | no       | -       | Appended as query parameters.               |
| `body`      | string  | no       | -       | Raw body. Ignored for GET and HEAD.         |
| `timeoutMs` | integer | no       | `20000` | Clamped to 1000-60000.                      |

## Response

| Field       | Notes                                      |
| ----------- | ------------------------------------------ |
| `url`       | Final URL after redirects.                 |
| `method`    | Method that was sent.                      |
| `status`    | HTTP status code.                          |
| `truncated` | `1` when the body was cut at 200000 bytes. |
| `headers`   | Response headers, one `name: value` line.  |
| `body`      | Response body text.                        |

See `response.toon`.

## Errors

- `http_request requires a non-empty `url`.`: missing url.
- `http_request must run on the async dispatch path.`: sync `execute`.
- `http_request method `<name>` is not supported.`: method outside the allow list.
- `http_request `<field>` must be an object.`: headers or query is not an object.
- `http_request URL is not parseable.`: bad URL.
- `http_request only accepts http and https URLs.`: other scheme.
- `http_request body exceeds 1 MB.`: body too large.
- `http_request failed: <error>`: network or timeout.
- `User denied the HTTP write.`: user chose Deny.

## Source

`src-tauri/src/tools/http_request.rs` - `spec()` / `execute()` / `execute_async()`. Transport: `src-tauri/src/tools/tool-utils/http_call.rs` `send()`. Confirm: `ensure_http_write()`.
