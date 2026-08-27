# Tool call and response - exact formats

For every tool the model can call, this file shows:

- The streaming chunk Rust emits to the frontend while the model is
  producing the call.
- The exact YAML the tool returns.
- The exact `tool` message that Rust sends back to the LLM in the next
  request.

The chunk format is defined in `src-tauri/src/chat.rs`:

```rust
struct ChatChunk {
    kind: String, // "reasoning" | "tool" | "content" | "question"
    text: String,
}
```

The `tool` chunk is built by `emit_tool_call`:

```rust
let argument = tool_call_argument(&call.name, &call.arguments);
let text = if argument.is_empty() {
    call.name.clone()
} else {
    format!("{}\n{argument}", call.name)
};
```

So the chunk text is `"<tool_name>\n<argument>"`, where `<argument>` is
the tool's arguments rendered as a single line (object args become a
JSON string). The frontend parser is `parseToolChunkText`:

```ts
const nl = text.indexOf("\n");
if (nl < 0) return { name: text.trim() };
return { name: text.slice(0, nl).trim(), argument: text.slice(nl + 1).trim() };
```

The tool result is sent to the LLM as a `tool` role message
(`openai_messages` in `src-tauri/src/chat.rs`):

```json
{
  "role": "tool",
  "tool_call_id": "<id>",
  "content": "<yaml below>"
}
```

The `content` is the YAML produced by `yaml_doc` in
`src-tauri/src/tools/mod.rs` (block strings are indented with two
spaces, plain strings are quoted only when required).

---

## `skill`

Call args:

```json
{ "name": "code-review" }
```

Streaming chunk (`ChatChunk`):

```json
{ "kind": "tool", "text": "skill\n{\"name\":\"code-review\"}" }
```

Tool result (skill found, body trimmed):

```yaml
name: code-review
path: ~/.k-agent/skills/code-review/SKILL.md
body: |
  # Code review

  Apply this checklist:
  ...
```

Result message to the LLM:

```json
{
  "role": "tool",
  "tool_call_id": "<id>",
  "content": "name: code-review\npath: ~/.k-agent/skills/code-review/SKILL.md\nbody: |\n  # Code review\n\n  Apply this checklist:\n  ..."
}
```

---

## `read`

Call args:

```json
{ "filePath": "./smoke/sample.txt" }
```

Streaming chunk:

```json
{ "kind": "tool", "text": "read\n{\"./smoke/sample.txt\"}" }
```

For `./smoke/sample.txt` containing exactly:

```
line A
line B
line C
```

Tool result:

```yaml
path: ./smoke/sample.txt
startLine: 1
endLine: 3
content: |
  1: line A
  2: line B
  3: line C

  (End of file - total 3 lines)
```

The `content` block is line-numbered (`<n>: <line>`). The frontend
strips the `<n>: ` prefix and the `(End of file ...)` footer before
rendering, and uses `startLine` for the editor gutter origin (see
`readToolView` in `src/features/chat/ToolCallsBlock.tsx`).

When the read is offset (e.g. `offset: 50, limit: 200`) and the file is
longer, the footer is one of:

```yaml
content: |
  50: <line>
  ...
  249: <line>

  (Showing lines 50-249 of 412. Use offset=250 to continue.)
```

```yaml
content: |
  50: <line>
  ...

  (Output capped at 50 KB. Showing lines 50-249. Use offset=250 to continue.)
```

---

## `write`

Call args:

```json
{ "filePath": "./src/foo.ts", "content": "export function foo() {\n  return 1;\n}\n" }
```

Streaming chunk:

```json
{
  "kind": "tool",
  "text": "write\n{\"./src/foo.ts\",\"content\":\"export function foo() {\\n  return 1;\\n}\\n\"}"
}
```

Tool result:

```yaml
path: ./src/foo.ts
status: ok
```

Note: the file is also captured to `sessions/{id}/files/{callId}.before`
and `.after` (see `write_file_revision` in `src-tauri/src/sessions.rs`).

---

## `edit`

Call args:

```json
{
  "filePath": "./src/foo.ts",
  "oldString": "  return 1;",
  "newString": "  return 2;",
  "replaceAll": false
}
```

Streaming chunk:

```json
{
  "kind": "tool",
  "text": "edit\n{\"./src/foo.ts\",\"oldString\":\"  return 1;\",\"newString\":\"  return 2;\",\"replaceAll\":false}"
}
```

Tool result:

```yaml
path: ./src/foo.ts
status: ok
added: 1
removed: 1
```

`added` / `removed` are line counts from a hunk-style diff
(`line_add_remove`). The file is also captured as before/after.

---

## `list_directory`

Call args:

```json
{ "dirPath": "./src" }
```

Streaming chunk:

```json
{ "kind": "tool", "text": "list_directory\n{\"./src\"}" }
```

For a workspace like:

```
src/
  app.tsx
  main.tsx
  components/
    Button.tsx
```

Tool result:

```yaml
path: ./src
entries: |
  app.tsx
  main.tsx
  components/
    Button.tsx
```

With `recursive: true, maxDepth: 2`:

```yaml
path: ./src
entries: |
  app.tsx
  main.tsx
  components/
    Button.tsx
```

(`maxDepth` clamps the walk. The default depth is 3 and the maximum is
10.)

---

## `ask_user`

Call args (the model emits a `question` chunk, not a tool call):

```json
{
  "questions": [
    {
      "id": "approach",
      "header": "Approach",
      "question": "Which approach should I take?",
      "options": [
        { "label": "Refactor in place", "description": "Keep the file shape." },
        { "label": "Extract helper", "description": "Move logic to a shared util." }
      ],
      "multiSelect": false,
      "allowFreeText": true
    }
  ]
}
```

Streaming chunk:

```json
{ "kind": "question", "text": "{\"callId\":\"call_abc\",\"questions\":[...]}" }
```

The frontend parses the JSON and opens `QuestionDialog`. The user
clicks an option (or types free text) and clicks Submit. The frontend
then invokes `submit_ask_user_answer`:

```json
{
  "callId": "call_abc",
  "answers": [{ "questionId": "approach", "selected": ["Extract helper"], "freeText": "" }]
}
```

If the user closes the dialog instead, the frontend invokes
`cancel_ask_user_answer` with the same `callId`. Either path emits a
synthetic tool result back to Rust:

```yaml
status: submitted
answers: |
  - questionId: approach
    selected: ["Extract helper"]
    freeText: ""
```

(or `status: cancelled` for cancel). That YAML is what the next LLM
turn sees as a `tool` role message.

---

## `create_folder`

Call args:

```json
{ "dirPath": "./src/widgets" }
```

Streaming chunk:

```json
{ "kind": "tool", "text": "create_folder\n{\"./src/widgets\"}" }
```

Tool result (newly created):

```yaml
path: ./src/widgets
status: created
```

If the folder already exists:

```yaml
path: ./src/widgets
status: exists
```

If the path points to an existing file, the tool errors with
`status: "error"`, `kind: "action"`, and a human-readable message in
`output`.

---

## `delete`

Call args:

```json
{ "path": "./src/old.ts" }
```

Streaming chunk:

```json
{ "kind": "tool", "text": "delete\n{\"./src/old.ts\"}" }
```

Tool result (file deleted):

```yaml
path: ./src/old.ts
status: deleted
kind: file
linesRemoved: 42
```

Tool result (empty directory deleted):

```yaml
path: ./src/empty
status: deleted
kind: directory
```

The frontend tracks `linesRemoved` per call and subtracts it from the
conversation token walk (see `context-usage.ts`).

---

# Worked round

One full round for the user message `What does the foo function do?`
in a workspace where `src/foo.ts` exists with:

```ts
export function foo() {
  return 1;
}
```

The frontend already sent the system prompt shown in
[`system-prompt.md`](./system-prompt.md) and the previous `user`
message. Rust has streamed no chunks yet. The first LLM response
streams in this order:

## 1. Reasoning chunks

Rust emits zero or more `reasoning` chunks as the model thinks. The
text is concatenated on the frontend in `round.reasoning`.

```json
{ "kind": "reasoning", "text": "The user is asking what the foo function does. " }
{ "kind": "reasoning", "text": "I should read the file first to see the implementation." }
```

## 2. Tool chunk

```json
{ "kind": "tool", "text": "read\n{\"./src/foo.ts\"}" }
```

## 3. Tool result

Rust executes the tool, produces the YAML, and on the next round (still
inside the same turn) feeds it back as a `tool` role message:

```yaml
path: ./src/foo.ts
startLine: 1
endLine: 3
content: |
  1: export function foo() {
  2:   return 1;
  3: }

  (End of file - total 3 lines)
```

## 4. Next round: reasoning

The model thinks again. The boundary `tool -> reasoning` increments
`activeRoundIndex` in the frontend chunk handler (see `sessions.ts`).

```json
{ "kind": "reasoning", "text": "Now I can answer. The foo function is a trivial constant." }
```

## 5. Final content

````json
{ "kind": "content", "text": "The foo function in `src/foo.ts` returns the constant `1`:\n\n" }
{ "kind": "content", "text": "```ts\nexport function foo() {\n  return 1;\n}\n```\n\nThat's all it does - no side effects, no arguments." }
````

## 6. Final result

After the streaming ends, Rust returns:

````json
{
  "content": "The foo function in `src/foo.ts` returns the constant `1`:\n\n```ts\nexport function foo() {\n  return 1;\n}\n```\n\nThat's all it does - no side effects, no arguments.",
  "reasoning": "The user is asking what the foo function does. I should read the file first to see the implementation. Now I can answer. The foo function is a trivial constant.",
  "reasoningSignature": "",
  "toolRounds": [
    {
      "reasoning": "The user is asking what the foo function does. I should read the file first to see the implementation.",
      "reasoningSignature": "",
      "content": "",
      "calls": [
        {
          "id": "call_xyz",
          "name": "read",
          "argument": "./src/foo.ts",
          "arguments": "{\"./src/foo.ts\"}",
          "thoughtSignature": "",
          "output": "path: ./src/foo.ts\nstartLine: 1\nendLine: 3\ncontent: |\n  1: export function foo() {\n  2:   return 1;\n  3: }\n\n  (End of file - total 3 lines)",
          "display": {
            "kind": "context",
            "path": "./src/foo.ts",
            "startLine": 1,
            "endLine": 3,
            "status": "ok"
          }
        }
      ],
      "thinkingMs": 1234
    },
    {
      "reasoning": "Now I can answer. The foo function is a trivial constant.",
      "reasoningSignature": "",
      "content": "The foo function in `src/foo.ts` returns the constant `1`:\n\n```ts\nexport function foo() {\n  return 1;\n}\n```\n\nThat's all it does - no side effects, no arguments.",
      "calls": [],
      "thinkingMs": 567
    }
  ]
}
````

The frontend replaces its streamed `toolRounds` with the authoritative
ones above. The user's chat now shows two thinking collapsibles (one
per round), one tool call (`read`), and the final markdown response.
Each collapsible shows its own `thinkingMs`.
