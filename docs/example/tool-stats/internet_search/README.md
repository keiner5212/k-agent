# internet_search

Search the public web and return titles, URLs, sites, and snippets. Bing is tried first; if Bing's results look off-topic, blocked, or unavailable, DuckDuckGo HTML is used as a fallback.

## Does

- Calls Bing with the same Chrome 153 navigation profile used by `fetch_url`, including a warm-up GET to `bing.com` so the search request carries a `Referer`, host cookies, and `sec-fetch-site: same-origin`.
- Falls back to DuckDuckGo HTML when Bing's results fail the relevance check, when Bing returns an `unusual traffic` or captcha response, or when Bing's network call errors. A separate browser session is used for DuckDuckGo so Bing cookies are not sent there.
- Unwraps Bing's `/ck/a` redirects (base64 + `-` / `_` substitution, including the `?!&&` query form) and DuckDuckGo's `/l/?uddg=` redirects, including protocol-relative `//duckduckgo.com` links, so result URLs are the canonical target.
- Drops tracker query params (`utm_*`, `fbclid`, `gclid`, `mc_eid`, `yclid`) from result URLs.
- Filters every result URL through the same guard used by `fetch_url`. With HTTP fetch off, only `https://` on port 443 is kept. With it on, public `http://` on port 80 is kept too. Loopback, private hosts, IP literals, and tracker params are removed.
- Runs a relevance check before accepting a page: requires 60 percent term coverage across results and at least 2 strong matches (terms in title or URL). Common stop words (`the`, `and`, `with`, ...) are ignored when picking the meaningful terms. If both engines fail that check, the page is dropped. Decoy SERPs are not returned.
- Caps pages at 2, results per page at 10. Pauses 1.5 to 3.5 s between pages.
- Persists a non-empty relevant search to the app data `cache/internet-search/{hash}.json` with a 1-hour TTL keyed by query, language, and page count. Empty and off-topic searches are not cached.

## Does not

- Run JavaScript on search-result pages. SPA-only engines (Ecosia live results, etc.) are not parsed.
- Fetch the result pages. The tool returns snippets only. Call `fetch_url` on a chosen URL to read the page. Snippets are not a substitute for that fetch.
- Return off-topic results. Bing sometimes answers a different query; those rows are dropped instead of shown.
- Return loopback, private, or IP-literal URLs. Intentional.
- Retry on access denial or captcha. Those errors end the search so the next call does not compound the rate limit.
- Sort results across pages. Within a page the order is the engine's natural order; across pages, dedup keeps the first occurrence per URL.

## Options

| Name    | Type    | Required | Default | Notes                                                                             |
| ------- | ------- | -------- | ------- | --------------------------------------------------------------------------------- |
| `query` | string  | yes      | -       | Trimmed to 200 chars; control characters become spaces.                           |
| `lang`  | string  | no       | `en-US` | Optional BCP 47 language tag. Sets the `Accept-Language` header on every request. |
| `pages` | integer | no       | `1`     | 1 or 2. Anything else falls back to 1.                                            |

## Response

| Field     | Type    | Notes                                                                                                                      |
| --------- | ------- | -------------------------------------------------------------------------------------------------------------------------- |
| `query`   | string  | Echo of the query as sent to the search engine.                                                                            |
| `lang`    | string  | Effective BCP 47 language.                                                                                                 |
| `pages`   | integer | Number of pages requested.                                                                                                 |
| `results` | integer | Total unique URLs returned.                                                                                                |
| `items`   | string  | Multi-line block with one bullet per result in the form `- [pos] title - url`, followed by `  snippet` and `  site: host`. |

When the cache is hit, the response also includes `cached: 1`. See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `internet_search requires a non-empty \`query\`.`/`\`lang\` is invalid.`/`\`pages\` must be 1 or 2.`
- `internet_search must run on the async dispatch path.`: invoked from the sync `Tool::execute` path.
- `Search blocked (429)`: Bing returned an `unusual traffic` or captcha response.
- `Search blocked`: DuckDuckGo returned an anomaly or challenge page.
- `fetch_url page refused the request.` (status 401, 402, 403, 407, 451) when both engines refuse.

## Source

`src-tauri/src/tools/internet_search.rs` - entry point: `InternetSearchTool::execute()` (sync, errors out for the async path only). Async entry: `execute_async()`. Bing + DuckDuckGo parsers, URL unwrap helpers, persistent cache. Shares `BrowserSession` and the SSRF guard with `fetch_url`.
