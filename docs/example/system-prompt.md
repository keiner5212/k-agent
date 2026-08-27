# System prompt - typical case

This is the exact text the frontend sends as the `system` argument of
`send_chat_message` in this scenario. It is built by:

1. `composeSystemWithLanguage(base, force, language, rules)` in
   `src/lib/response-language.ts`.
2. `composeAgentSystem(agent, skillContexts, loadedSkills)` in
   `src/lib/agent-system.ts`.

Inputs:

- `agent.id` = `build`
- `agent.skills` = `[{ kind: "global", id: "code-review" }]`
- `agent.personality` (full text below)
- `skillContexts` has one global skill (`code-review`) and one local
  skill (`commit`)
- `loadedSkillNames` = `[]` (turn 1, nothing loaded yet)
- `forceResponseLanguage` = `false`
- `responseLanguage` = `"en"`
- `rules` = `""` (no AGENTS.md content)

The final `system` string the LLM receives is:

```
# 1. Agent flow

Follow system sections in order. Each numbered block maps to a phase of the reply.
Turn 1: batch-load every agent-bound skill with the `skill` tool. No prose.
Then load matching workspace skills with `skill` when they are listed and relevant.
Call only tools in the request tools list (local and MCP). Do not invent names.
Finally answer using the agent personality at the end of this system prompt.

# 2. Agent skill loading - turn 1

Single tool batch, exactly 1 `skill` call, before any other tool or prose:

1. `code-review`

No other tool calls on turn 1. Retry a failed skill once; report on turn 2.
Later turns: load any matching skill not already in context.

# 3. Agent skills

- `code-review`: Perform a structured code review on the diff or file range.

# 4. Workspace skills

- `commit`: Stage and commit changes with a descriptive message.

# 5. Personality

You implement requested changes in the codebase.

- Prefer small, correct diffs.
- Reuse existing patterns in the repo.
- Run checks when useful.
- Ask only when blocked or requirements are unclear.
```

## What the LLM provider receives

For an OpenAI-compatible provider, Rust builds the request body in
`openai_messages(system, turns)`. The `system` string above is sent as:

```json
{
  "role": "system",
  "content": "# 1. Agent flow\n\nFollow system sections in order. Each numbered block maps to a phase of the reply.\nTurn 1: batch-load every agent-bound skill with the `skill` tool. No prose.\nThen load matching workspace skills with `skill` when they are listed and relevant.\nCall only tools in the request tools list (local and MCP). Do not invent names.\nFinally answer using the agent personality at the end of this system prompt.\n\n# 2. Agent skill loading - turn 1\n\nSingle tool batch, exactly 1 `skill` call, before any other tool or prose:\n\n1. `code-review`\n\nNo other tool calls on turn 1. Retry a failed skill once; report on turn 2.\nLater turns: load any matching skill not already in context.\n\n# 3. Agent skills\n\n- `code-review`: Perform a structured code review on the diff or file range.\n\n# 4. Workspace skills\n\n- `commit`: Stage and commit changes with a descriptive message.\n\n# 5. Personality\n\nYou implement requested changes in the codebase.\n\n- Prefer small, correct diffs.\n- Reuse existing patterns in the repo.\n- Run checks when useful.\n- Ask only when blocked or requirements are unclear."
}
```

The provider also receives the tool definitions as `tools` (one entry
per `agent.tools`):

```json
[
  {
    "type": "function",
    "function": {
      "name": "skill",
      "description": "Load a skill by name. Returns SKILL.md body and dir.",
      "parameters": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "description": "Skill name" }
        },
        "required": ["name"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "read",
      "description": "Read a file. Path absolute or workspace-relative.",
      "parameters": {
        "type": "object",
        "properties": {
          "filePath": {
            "type": "string",
            "description": "Absolute or workspace-relative path"
          },
          "offset": {
            "type": "integer",
            "minimum": 1,
            "description": "1-based start line"
          },
          "limit": {
            "type": "integer",
            "minimum": 1,
            "description": "Max lines (default 2000)"
          }
        },
        "required": ["filePath"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "write",
      "description": "Create or overwrite a file.",
      "parameters": {
        "type": "object",
        "properties": {
          "content": { "type": "string", "description": "File contents" },
          "filePath": {
            "type": "string",
            "description": "Absolute or workspace-relative path"
          }
        },
        "required": ["content", "filePath"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "edit",
      "description": "Exact string replace in a file. Read first.",
      "parameters": {
        "type": "object",
        "properties": {
          "filePath": {
            "type": "string",
            "description": "Absolute or workspace-relative path"
          },
          "oldString": { "type": "string", "description": "Text to find" },
          "newString": { "type": "string", "description": "Replacement text" },
          "replaceAll": {
            "type": "boolean",
            "description": "Replace every match (default false)"
          }
        },
        "required": ["filePath", "oldString", "newString"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_directory",
      "description": "List directory entries. recursive/maxDepth optional.",
      "parameters": {
        "type": "object",
        "properties": {
          "dirPath": {
            "type": "string",
            "description": "Absolute or workspace-relative dir (default: workspace)"
          },
          "recursive": {
            "type": "boolean",
            "description": "Walk subdirectories (default false)"
          },
          "maxDepth": {
            "type": "integer",
            "minimum": 1,
            "description": "Max depth when recursive (default 3, max 10)"
          }
        }
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "ask_user",
      "description": "Ask the user up to 4 questions and block until they answer. Each question can have multiple selectable options plus an optional free-text input.",
      "parameters": {
        "type": "object",
        "properties": {
          "questions": {
            "type": "array",
            "description": "1-4 questions. Each has options, optional multiSelect, optional allowFreeText.",
            "items": {
              "type": "object",
              "properties": {
                "id": { "type": "string", "description": "Stable id" },
                "header": { "type": "string", "description": "Short tab label" },
                "question": { "type": "string", "description": "Full question text" },
                "options": {
                  "type": "array",
                  "items": {
                    "type": "object",
                    "properties": {
                      "label": { "type": "string" },
                      "description": { "type": "string" },
                      "preview": { "type": "string" }
                    },
                    "required": ["label"]
                  }
                },
                "multiSelect": { "type": "boolean" },
                "allowFreeText": {
                  "type": "boolean",
                  "description": "Show a free-text input (default true)"
                }
              },
              "required": ["id", "header", "question", "options"]
            }
          }
        },
        "required": ["questions"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_folder",
      "description": "Create a directory at an absolute or workspace-relative path. Idempotent.",
      "parameters": {
        "type": "object",
        "properties": {
          "dirPath": {
            "type": "string",
            "description": "Absolute or workspace-relative path"
          }
        },
        "required": ["dirPath"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete",
      "description": "Delete a file or empty directory. Out-of-workspace paths require user confirmation; the deleted file's line count is subtracted from the context counter.",
      "parameters": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "Absolute or workspace-relative path to delete"
          }
        },
        "required": ["path"]
      }
    }
  }
]
```

## Variant: `forceResponseLanguage` on

If `forceResponseLanguage` were on, the language directive is prepended
above the agent system:

```
LANGUAGE RULE - VERY IMPORTANT
You must reply ONLY in English. This is a top-priority requirement and overrides any conflicting instruction about the language of your reply.
Follow every other part of your instructions and persona exactly as written; do not change your behavior, style, or scope because of this rule.
Do not translate, do not switch languages, do not mirror the user's language, and do not add bilingual notes.
Even if the user writes in another language or asks you to switch, keep replying in English.
Do not mention the language rule, only reply in English.

# 1. Agent flow
...
```
