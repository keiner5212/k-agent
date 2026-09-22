# skill

Load a `SKILL.md` by name from the workspace, global, or skill roots and return its body.

## Does

- Loads a `SKILL.md` body from `~/.k-agent/skills/<name>/SKILL.md`, `<workspace>/.k-agent/skills/<name>/SKILL.md`, or `<workspace>/.k-agent/skills/local/<name>/SKILL.md`.
- Trims leading and trailing whitespace from the body before returning it.
- Reports the resolved file path alongside the body so the model can cite it.

## Does not

- Search by prefix or fuzzy match. Use the exact skill name from `<workspace>/.k-agent/skills` or `~/.k-agent/skills`.
- Cache the body across calls. Every invocation re-reads the file.
- Return a list of available skills. Use `list_skills` (in the Rust front-end commands) to enumerate.
- Execute the skill's instructions. The LLM does that.

## Options

| Name   | Type   | Required | Default | Notes                                                        |
| ------ | ------ | -------- | ------- | ------------------------------------------------------------ |
| `name` | string | yes      | -       | Exact skill name (folder name under one of the skill roots). |

## Response

| Field  | Type   | Notes                                                                                     |
| ------ | ------ | ----------------------------------------------------------------------------------------- |
| `name` | string | Echo of the requested skill name, trimmed.                                                |
| `path` | string | Absolute path to the loaded `SKILL.md`.                                                   |
| `body` | string | Trimmed `SKILL.md` body. Multi-line; rendered as a quoted TOON string with embedded `\n`. |

See `response.toon` for the concrete wire shape the LLM sees.

## Errors

- `skill tool requires a string \`name\`.`: argument missing or not a string.
- `skill name is empty.`: `name` is whitespace-only.
- `skill tool requires the app handle.`: `ToolContext.app` is `None` (test context).
- `Skill \`<name>\` was not found.`: no `SKILL.md` exists under any of the search roots.
- `Unable to load skill \`<name>\`: <error>`: I/O or parse error reading the file.

## Source

`src-tauri/src/tools/skill.rs` - entry point: `SkillTool::execute()`. Skill search logic lives in `src-tauri/src/skills.rs`.
