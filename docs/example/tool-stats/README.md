# Tool examples

Auto-generated and hand-written reference for every tool the LLM can call.
One folder per tool. The Rust integration test
[`src-tauri/tests/tools_examples.rs`](../../../src-tauri/tests/tools_examples.rs)
drives each tool against a real or scratch workspace and dumps the artifacts
below. Re-run it whenever a tool gains a field, a new option, or the wire
format changes.

## How to regenerate

From the repo root:

```
cargo test --test tools_examples -- --nocapture
```

The `--nocapture` flag prints the per-tool `[tool-examples]` summary to
stderr. The test is always-on: it runs with `cargo test` without any env
flag, but it only writes files; it does not fail unless a tool panics.

What the test does, in order:

1. Creates a scratch directory at `docs/.tmp/tool-examples-<pid>-<nanos>/`
   (gitignored via the `.tmp` rule). Mutating tools - `create_folder`,
   `write`, `edit`, `delete` - run with that scratch directory as their
   workspace.
2. Runs `skill`, `read`, and `list_directory` against the real
   `docs/` workspace so the response surfaces actual project content.
3. Writes the four artifacts below into each tool folder.
4. Removes the scratch directory so the repo tree stays clean.

## Per-tool layout

```
docs/example/tool-stats/<tool>/
  README.md      hand-written specification (Does / Does not / Options / Response / Errors / Source)
  input.json     the JSON arguments the test passed to execute(), pretty-printed
  response.toon  the tool's response in the same TOON the LLM sees on the wire
  stats.md       timing (wall / user CPU / sys CPU in microseconds), memory (RSS before / after / delta / host peak), parallelism (configured cores, host cores), response size (bytes / chars / lines / tokens), and the tool's outcome
```

- `input.json` is the exact JSON object the dispatcher would forward to
  the tool. Pretty-printed so reviewers can diff changes between runs.
- `response.toon` matches the wire format the LLM receives in its next
  `tool` role message verbatim. Multi-line strings are quoted with `\n`
  escapes per TOON spec v4.1 (decided by the `toon-format` crate).
- `stats.md` includes a RSS delta across the call plus the process
  lifetime peak from `getrusage`. The peak is large because the test
  binary has already loaded every shared library; the delta is the real
  per-tool attribution. The exact wording is in the file's
  `## Memory` section.

## Conventions

- Hand-written `README.md` files are prettier-checked.
- Auto-generated `input.json`, `response.toon`, and `stats.md` are
  excluded from prettier in `.prettierignore`. Their widths come from
  Rust `format!` strings, not from prettier's print width, and
  re-formatting them every run produces noisy diffs.
- ASCII punctuation only. No smart quotes, em/en dashes, typographic
  arrows, or emoji. This matches the rest of the repo.
- All text is regenerated deterministically except the timestamp and
  CPU brand string. Two back-to-back runs produce byte-identical files
  apart from those fields and the per-process scratch directory name.

## Adding a new tool

The full convention lives in `AGENTS.md` under the "Tools" heading.
Short version:

1. Implement `src-tauri/src/tools/<name>.rs`, register the constant
   `pub const NAME: &str = "<name>";`, and export
   `<NAME>_TOOL_NAME` from `src-tauri/src/tools/mod.rs`.
2. Add a block to `src-tauri/tests/tools_examples.rs` that builds the
   args, calls `execute(...)` or the async entry, and runs `write_input`
   - `write_stats`.
3. Write `docs/example/tool-stats/<name>/README.md` using the template
   in `AGENTS.md` (Does / Does not / Options / Response / Errors / Source).
4. Run `cargo test --test tools_examples` and commit the four generated
   artifacts along with the source change.
