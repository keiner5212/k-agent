# Update the model catalog

Re-run this prompt when `src-tauri/catalog/models.json` is stale. Do this work. Do not only propose a plan.

Read `docs/model-catalog.md` and `src-tauri/src/catalog.rs` (`CatalogEntry`, `ModelRequestSpec`, `ModelReasoningSpec`) before editing. The bundled file is the list of models the app knows, matched by `id` or `aliases`, plus the wire contract in `request`. `https://models.dev/api.json` refreshes limits at runtime. It does not add models and it does not replace `request`. Do not use models.dev, OpenRouter, blogs, or leaderboards as a source.

## Source rule

Every changed number, flag, modality, or new id comes from that provider's official docs, opened in this run. Record the URL next to the change. If the official page cannot be opened, leave that provider unchanged.

Start from these official lists. Add a provider only when this repo already has its models and you opened its docs:

- OpenAI: https://platform.openai.com/docs/models and https://platform.openai.com/docs/pricing
- Anthropic: https://platform.claude.com/docs/en/about-claude/models and https://platform.claude.com/docs/en/about-claude/pricing
- Google Gemini: https://ai.google.dev/gemini-api/docs/models
- DeepSeek: https://api-docs.deepseek.com/quick_start/pricing
- Mistral: https://docs.mistral.ai/getting-started/models/models_overview
- xAI: https://docs.x.ai/docs/models
- Groq: https://console.groq.com/docs/models
- MiniMax: official platform model docs (the page that lists model ids, context, and price)

## What to change

- Add a model the official list currently offers and `models.json` lacks. Match the existing entry shape. `request.known` and `request.native` are required.
- Update `contextWindow`, `maxOutputTokens`, `knowledge`, `input`, `output`, `cost`, and capability flags only when the official page states them. Cost is USD per million tokens, same units as the neighboring entries.
- `request.known: true` only when an official request example matches a protocol in `native` (`openai`, `anthropic`, `gemini`, or a host the catalog already uses, such as `minimax`). Otherwise `known: false` and `native` set to the provider protocol. Do not copy `request` from a sibling model. Do not invent `reasoning`, `sampling`, `tiers`, or `tokenField`.
- New alias when the official id renamed and the old id must still match. Do not delete an id. A saved provider model may still use it.
- Leave Cloudflare and other gateway dumps alone unless that vendor's own docs were opened.

## How to edit

Patch entries. Do not rewrite the file. Keep existing key order. Insert new objects in `id` sort order. ASCII only.

After the edit:

- Set `Refreshed on YYYY-MM-DD` in `docs/model-catalog.md` to today.
- Parse the file. `models.json` is `include_str!` and must deserialize as `Vec<CatalogEntry>`. Run the catalog tests with repo-local `CARGO_TARGET_DIR` and `TMPDIR` (`.tmp` at the repo root, never `/tmp`).
- Summarize added ids, updated fields, and untouched providers whose docs were not opened.
