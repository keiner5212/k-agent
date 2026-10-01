# graphql

Send one GraphQL request and return the HTTP status, headers, and body.

## Does

- Requires `url` and `query`. `variables` and `operationName` are optional.
- Default method is `POST`. The JSON body is `{ query, variables?, operationName? }`.
- Adds `content-type: application/json` when the caller did not set one.
- `GET` and `HEAD` put `query`, `operationName`, and `variables` on the query string and send no body.
- Accepts any `http` or `https` endpoint, including localhost.
- Uses the same limits as `http_request`: body `1 MB`, response `200000` bytes, timeout `1000-60000` ms, 5 redirects.
- `GET` and `HEAD` run immediately. Any other method waits on Deny, Accept this time, or Accept for this chat.
- Accept for this chat stores `httpWriteAllowed` on the session. It does not grant filesystem access.
- Response body is lossy UTF-8, same as `http_request`.

## Does not

- Validate the GraphQL document. The endpoint does that.
- Read an HTML page. Use `fetch_url` to read a public page.
- Send arbitrary REST bodies. Use `http_request` for that.
- Attach a screenshot. Use `page_shot` for pixels.

## Options

| Name            | Type    | Required | Default | Notes                                   |
| --------------- | ------- | -------- | ------- | --------------------------------------- |
| `url`           | string  | yes      | -       | GraphQL endpoint. http or https.        |
| `query`         | string  | yes      | -       | Query or mutation document.             |
| `variables`     | object  | no       | -       | JSON variables.                         |
| `operationName` | string  | no       | -       | Operation name.                         |
| `headers`       | object  | no       | -       | Extra headers.                          |
| `method`        | string  | no       | `POST`  | `GET` skips confirm. Other methods ask. |
| `timeoutMs`     | integer | no       | `20000` | Clamped to 1000-60000.                  |

## Response

| Field       | Notes                                      |
| ----------- | ------------------------------------------ |
| `url`       | Final URL after redirects.                 |
| `method`    | Method that was sent.                      |
| `status`    | HTTP status code.                          |
| `truncated` | `1` when the body was cut at 200000 bytes. |
| `headers`   | Response headers, one `name: value` line.  |
| `body`      | Response body text, usually JSON.          |

See `response.toon`.

## Errors

- `graphql requires non-empty `url`and`query`.`: missing field.
- `graphql must run on the async dispatch path.`: sync `execute`.
- `http_request method `<name>` is not supported.`: method outside the allow list.
- `http_request `<field>` must be an object.`: headers is not an object.
- `http_request failed: <error>`: network or timeout.
- `User denied the HTTP write.`: user chose Deny.

## Source

`src-tauri/src/tools/graphql.rs` - `spec()` / `execute()` / `execute_async()`. Transport: `src-tauri/src/tools/tool-utils/http_call.rs` `send()`. Confirm: `ensure_http_write()`.
