import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { LineKind } from "@/components/LineEditor";
import { ReadOnlyEditorDialog } from "@/features/chat/ReadOnlyEditorDialog";
import { TodoList } from "@/features/chat/TodoList";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { diffEditorValue, readSessionFileRevision, toonFieldValue } from "@/lib/session-files";
import { skillNameFromCall, type ChatToolCall } from "@/types/chat";
import { formatContextWindow } from "@/types/providers";

type ToolCallsBlockProps = {
  calls: ChatToolCall[];
  sessionId: string | null;
};

type PreviewState = {
  titleKey: string;
  value: string;
  path?: string;
  startLine?: number;
  lineNumbers?: number[];
  lineKinds?: LineKind[];
};

const fileName = (path: string): string => {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
};

const TOOL_TITLE: Record<string, string> = {
  skill: "chat.tools.skillTitle",
  read: "chat.tools.readTitle",
  list_directory: "chat.tools.listTitle",
  write: "chat.tools.writeTitle",
  edit: "chat.tools.editTitle",
  create_folder: "chat.tools.createFolderTitle",
  delete: "chat.tools.deleteTitle",
  todowrite: "chat.tools.todoTitle",
  validate_mermaid: "chat.tools.validateMermaidTitle",
  bash: "chat.tools.bashTitle",
  grep: "chat.tools.grepTitle",
  fetch_url: "chat.tools.fetchTitle",
  internet_search: "chat.tools.searchTitle",
  http_request: "chat.tools.httpTitle",
  graphql: "chat.tools.graphqlTitle",
  page_shot: "chat.tools.shotTitle",
  ask_user: "chat.tools.askTitle",
};

const TOOL_SYMBOL: Record<string, string> = {
  skill: "\u2726 ",
  read: "\u25CB ",
  write: "\u270E ",
  edit: "\u2710 ",
  list_directory: "\u229E ",
  ask_user: "\u25CC ",
  create_folder: "\u25A4 ",
  delete: "\u2715 ",
  todowrite: "\u25C7 ",
  validate_mermaid: "\u25A1 ",
  bash: "\u25B8 ",
  grep: "\u2315 ",
};

const READ_LINE_RE = /^(\d+): (.*)$/;
const END_OF_FILE_RE = /^\(End of file.*\)$/;

const readToolView = (raw: string): { content: string; startLine: number | undefined } => {
  const source = toonFieldValue(raw, "content") || raw;
  const cleaned: string[] = [];
  let startLine: number | undefined;
  let sawNumbered = false;
  for (const line of source.split("\n")) {
    const match = line.match(READ_LINE_RE);
    if (match) {
      if (!sawNumbered) {
        startLine = Number(match[1]);
        sawNumbered = true;
      }
      cleaned.push(match[2] ?? "");
    } else {
      cleaned.push(line);
    }
  }
  while (cleaned.length > 0) {
    const last = cleaned[cleaned.length - 1] ?? "";
    if (END_OF_FILE_RE.test(last) || last === "") {
      cleaned.pop();
      continue;
    }
    break;
  }
  return { content: cleaned.join("\n"), startLine };
};

const argString = (call: ChatToolCall, key: string): string => {
  const raw = call.arguments ?? (call.argument?.startsWith("{") ? call.argument : undefined);
  if (!raw) return "";
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const value = parsed[key];
    return typeof value === "string" ? value.trim() : "";
  } catch {
    return "";
  }
};

const fieldOr = (raw: string, key: string, fallback: string): string => {
  const value = toonFieldValue(raw, key);
  return value.length > 0 ? value : fallback;
};

const previewFromOutput = (call: ChatToolCall): string => {
  const raw = call.output ?? "";
  const error = toonFieldValue(raw, "error");
  if (call.name === "skill") return fieldOr(raw, "body", error || raw);
  if (call.name === "list_directory") return fieldOr(raw, "entries", error || raw);
  if (call.name === "grep") return fieldOr(raw, "matches", error || raw);
  if (call.name === "internet_search") return fieldOr(raw, "items", error || raw);
  if (call.name === "http_request" || call.name === "graphql") {
    return fieldOr(raw, "body", error || raw);
  }
  if (call.name === "fetch_url") {
    if (error) return error;
    const title = toonFieldValue(raw, "title");
    const content = toonFieldValue(raw, "content");
    if (title && content) return `${title}\n\n${content}`;
    return content || raw;
  }
  if (call.name === "ask_user")
    return fieldOr(raw, "answers", toonFieldValue(raw, "status") || raw);
  if (call.name === "validate_mermaid") return error || toonFieldValue(raw, "status") || raw;
  if (call.name === "page_shot") return error || toonFieldValue(raw, "url") || raw;
  if (call.name === "todowrite") {
    const todos = call.display?.todos;
    if (todos && todos.length > 0) {
      return todos.map((item) => `${item.status}  ${item.content}`).join("\n");
    }
    return fieldOr(raw, "summary", error || raw);
  }
  if (call.name === "bash") {
    const command = argString(call, "command") || call.argument?.trim() || "";
    const output = toonFieldValue(raw, "output");
    const exitCode = toonFieldValue(raw, "exitCode");
    const lines: string[] = [];
    if (command) lines.push(`$ ${command}`);
    if (exitCode) lines.push(`exit ${exitCode}`);
    const body = output || error;
    if (body) lines.push(body);
    return lines.join("\n") || raw;
  }
  return error || raw;
};

const toolCallLabel = (call: ChatToolCall, lineRange = ""): string => {
  const name = call.name;
  if (name === "skill") {
    const skill = skillNameFromCall(call);
    return skill ? `${name} "${skill}"` : name;
  }
  const path = call.display?.path?.trim() || call.argument?.trim() || "";
  if (name === "read") {
    if (!path) return name;
    return lineRange ? `${name} "${path}" ${lineRange}` : `${name} "${path}"`;
  }
  if (name === "list_directory") return path ? `${name} "${path}"` : name;
  if (name === "bash") {
    const command = argString(call, "command");
    if (!command) return name;
    const shown = command.length > 80 ? `${command.slice(0, 77)}...` : command;
    return `${name} "${shown}"`;
  }
  if (name === "grep") {
    const pattern = argString(call, "pattern");
    return pattern ? `${name} "${pattern}"` : name;
  }
  if (call.display?.kind === "action") {
    return path ? `${name} ${fileName(path)}` : name;
  }
  if (path) return `${name} "${path}"`;
  return name;
};

const ToolCallsBlock = ({ calls, sessionId }: ToolCallsBlockProps): ReactNode => {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<PreviewState | null>(null);
  if (calls.length === 0) return null;

  const openPreview = async (call: ChatToolCall): Promise<void> => {
    const display = call.display;
    const fileEdit = call.name === "write" || call.name === "edit";
    if (fileEdit && display?.kind === "action" && call.id && sessionId) {
      if (display.status !== "ok") {
        setPreview({
          titleKey: call.name === "edit" ? "chat.tools.editTitle" : "chat.tools.writeTitle",
          value: toonFieldValue(call.output ?? "", "error") || t("chat.tools.error"),
          path: display.path,
        });
        return;
      }
      try {
        const after = await readSessionFileRevision(sessionId, call.id, "after");
        if (call.name === "write") {
          setPreview({
            titleKey: "chat.tools.writeTitle",
            value: after.content,
            path: display.path,
            startLine: 1,
          });
          return;
        }
        const before = await readSessionFileRevision(sessionId, call.id, "before");
        const packed = diffEditorValue(before.content, after.content);
        setPreview({
          titleKey: "chat.tools.editTitle",
          value: packed.value,
          path: display.path,
          lineNumbers: packed.lineNumbers,
          lineKinds: packed.lineKinds,
        });
        return;
      } catch {
        setPreview({
          titleKey: TOOL_TITLE[call.name] ?? "chat.tools.outputTitle",
          value: previewFromOutput(call),
          path: display.path,
        });
        return;
      }
    }
    const titleKey = TOOL_TITLE[call.name] ?? "chat.tools.outputTitle";
    const raw = call.output ?? "";
    const parsedRead =
      call.name === "read" && toonFieldValue(raw, "content") ? readToolView(raw) : null;
    setPreview({
      titleKey,
      value: parsedRead?.content ?? previewFromOutput(call),
      path: display?.path,
      startLine: parsedRead ? (parsedRead.startLine ?? display?.startLine) : undefined,
    });
  };

  return (
    <>
      <ul className="chat-tools">
        {calls.map((call, index) => {
          const display = call.display;
          const output = call.output?.trim() ?? "";
          const tokens = output.length > 0 ? estimateTokensFromText(output) : null;
          const isAction = display?.kind === "action";
          const lineRange =
            display?.startLine !== undefined && display.endLine !== undefined
              ? t("chat.tools.lines", { start: display.startLine, end: display.endLine })
              : "";
          const label = toolCallLabel(call, lineRange);
          const canOpen = Boolean(call.output) || Boolean(isAction && call.id);
          return (
            <li
              key={call.id ?? `${call.name}-${index}`}
              className={`chat-tools__item${call.name === "skill" ? " chat-tools__item--skill" : ""}${isAction ? " chat-tools__item--action" : ""}`}
            >
              <span className="chat-tools__line">
                <span className="chat-tools__symbol" aria-hidden="true">
                  {TOOL_SYMBOL[call.name] ?? ""}
                </span>
                {canOpen ? (
                  <button
                    type="button"
                    className="chat-tools__name"
                    onClick={() => {
                      void openPreview(call);
                    }}
                  >
                    {label}
                  </button>
                ) : (
                  <span>{label}</span>
                )}
                {isAction ? (
                  <span
                    className={`chat-tools__status${display?.status === "ok" ? " chat-tools__status--ok" : " chat-tools__status--error"}`}
                  >
                    {display?.status === "ok" ? t("chat.tools.ok") : t("chat.tools.error")}
                  </span>
                ) : null}
                {isAction && display?.added !== undefined && display.removed !== undefined ? (
                  <span className="chat-tools__diff">
                    <span className="change-bar__added">+{display.added}</span>
                    <span className="change-bar__removed">-{display.removed}</span>
                  </span>
                ) : null}
                {!isAction && tokens !== null ? (
                  <span className="chat-tools__tokens">~{formatContextWindow(tokens)}</span>
                ) : null}
              </span>
              {display?.imageData ? (
                <img
                  className="chat-tools__shot"
                  alt=""
                  src={`data:image/png;base64,${display.imageData}`}
                />
              ) : null}
              {call.name === "todowrite" && display?.todos && display.todos.length > 0 ? (
                <TodoList todos={display.todos} />
              ) : null}
            </li>
          );
        })}
      </ul>
      <ReadOnlyEditorDialog
        open={preview !== null}
        titleKey={preview?.titleKey ?? "chat.tools.outputTitle"}
        value={preview?.value ?? ""}
        path={preview?.path}
        startLine={preview?.startLine}
        lineNumbers={preview?.lineNumbers}
        lineKinds={preview?.lineKinds}
        onOpenChange={(open) => {
          if (!open) setPreview(null);
        }}
      />
    </>
  );
};

export { ToolCallsBlock };
export type { ToolCallsBlockProps };
