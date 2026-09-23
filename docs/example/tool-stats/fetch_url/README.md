# fetch_url

Fetch one public HTTPS page and return its title, description, main text, and safe outbound links. Mimics a Chrome 153 desktop navigation with realistic pacing, manual redirect handling, host-only cookies, and SSRF guards.

## Does

- Reads one public URL. HTTPS and port 443 by default. Public HTTP on port 80 is accepted only when `httpFetchEnabled` is on. Loopback, private hosts, and IP literals stay refused.
- Returns the canonical URL plus title (max 300 chars), description (max 500), main content (max 16 000), and up to 20 outbound links. Outbound links follow the same HTTP policy as the fetch.
- Decodes HTML entities and walks the DOM. Drops nav, footer, header, script, style, figures, and embed images before reading text.
- Picks the main block from `main`, `article`, or common article containers, then walks the DOM. Nested tags stay intact. Drops nav, footer, figures, embed images, and other chrome before reading text.
- Persists every successful fetch to the app data `cache/fetch-url/{hash}.json` with a 1-hour TTL. Subsequent calls hit the cache before touching the network.
- Jitters 50 to 250 ms before each GET so requests are not perfectly periodic. Retries one transient response (`408`, `429`, `500`, `502`, `503`, `504`).

## Does not

- Refuse IP literals, private or metadata hostnames. Use `read` against the workspace instead.
- Follow links or run JavaScript. Pages that render with client-side JS look empty. Use the LLM to read the raw HTML if needed.
- Cache negative results. A failed fetch retries on the next call.
- Decode content other than UTF-8 bytes. Binary responses error out with `fetch_url page is not HTML.`.
- Allow redirects to private hosts. The redirect target is re-validated against the same HTTPS+SSRF rules as the initial URL.
- Render emoji correctly inside TOON when the source uses surrogate halves. Non-BMP characters are filtered to ``.

## Options

| Name   | Type   | Required | Default | Notes                                                                                                         |
| ------ | ------ | -------- | ------- | ------------------------------------------------------------------------------------------------------------- |
| `url`  | string | yes      | -       | Public URL. HTTPS and port 443 by default. Public HTTP on port 80 only when HTTP fetch is enabled. Credentials, IP literals, loopback, and private hostnames are refused. |
| `lang` | string | no       | `en-US` | Optional BCP 47 language tag. Sets the `Accept-Language` header and adjusts `Accept-Language` quality values. |

## Response

| Field         | Type   | Notes                                                                                                                   |
| ------------- | ------ | ----------------------------------------------------------------------------------------------------------------------- |
| `url`         | string | Canonical URL after redirects.                                                                                          |
| `title`       | string | Up to 300 chars; prefers `og:title`, falls back to `<title>` then `<h1>`.                                               |
| `description` | string | Up to 500 chars; prefers `meta description` / `og:description`, falls back to first paragraph over 80 chars.            |
| `content`     | string | Up to 16 000 chars; stripped of navigation, footer, and other noise. Multi-line block.                                  |
| `links`       | string | Up to 20 article links, formatted as `- text (url)` lines. Skips `javascript:`, `http://`, image files, and chrome labels such as `Enlarge Image`. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `fetch_url requires a non-empty \`url\`.`: argument missing or blank.
- `fetch_url URL exceeds 2048 chars.`
- `fetch_url URL has leading or trailing whitespace.`
- `fetch_url URL contains control characters.`
- `fetch_url only accepts HTTP and HTTPS URLs.`
- `fetch_url HTTP is disabled. Enable HTTP fetch in settings.`
- `fetch_url only accepts the default port (443 for HTTPS, 80 for HTTP).`
- `fetch_url URL must not carry credentials.`
- `fetch_url refused: internal or private hostname.` / `IP literals are not allowed.`
- `fetch_url exceeded the redirect hop limit.`
- `fetch_url rate limited.` (HTTP 429 after the retry)
- `fetch_url page refused the request.` (HTTP 401, 402, 403, 407, 451)
- `fetch_url page is not HTML.` (content-type mismatch)
- `fetch_url response exceeded 1.5 MB.`
- `fetch_url page temporarily unavailable.` (HTTP 502/503/504 after the retry)
- `fetch_url request timed out.` (12 s timeout)

## Source

`src-tauri/src/tools/fetch_url.rs` - entry point: `FetchUrlTool::execute()` (sync, errors out for the async path only). Async entry: `execute_async()`. Browser session in the same file. DOM reader in `src-tauri/src/tools/tool-utils/readable.rs`.
