import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Dialog } from "@/components/Dialog";
import type { LineKind } from "@/components/LineEditor";
import { ReadOnlyEditorDialog } from "@/features/chat/ReadOnlyEditorDialog";
import { resolveVisionModel, toolCallTokens } from "@/lib/context-usage";
import { runDiffLinesJob } from "@/lib/jobs";
import { useProvidersStore } from "@/lib/providers";
import { readSessionFileRevision, toonFieldValue } from "@/lib/session-files";
import { useSelectionStore } from "@/lib/selected-model";
import {
  skillNameFromCall,
  type ChatMessage,
  type ChatToolCall,
  type ToolDisplay,
} from "@/types/chat";
import type { SessionRecord } from "@/types/sessions";
import { formatContextWindow } from "@/types/providers";
import { ChatMarkdown } from "./ChatMarkdown";

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
  language?: string;
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
  bash: "chat.tools.bashTitle",
  background: "chat.tools.backgroundTitle",
  grep: "chat.tools.grepTitle",
  fetch_url: "chat.tools.fetchTitle",
  internet_search: "chat.tools.searchTitle",
  http_request: "chat.tools.httpTitle",
  graphql: "chat.tools.graphqlTitle",
  page_shot: "chat.tools.shotTitle",
  ask_user: "chat.tools.askTitle",
  apply_patch: "chat.tools.patchTitle",
  lsp: "chat.tools.lspTitle",
  task: "chat.tools.taskTitle",
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
  bash: "\u25B8 ",
  background: "\u25B8 ",
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
    if (typeof parsed[key] !== "string") return "";
    return parsed[key].trim();
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
  if (call.name === "page_shot") return error || toonFieldValue(raw, "url") || raw;
  if (call.name === "todowrite") {
    const todos = call.display?.todos;
    if (todos && todos.length > 0) {
      return todos.map((item) => `${item.status}  ${item.content}`).join("\n");
    }
    return fieldOr(raw, "summary", error || raw);
  }
  if (call.name === "bash" || call.name === "background") {
    const command = argString(call, "command") || call.argument?.trim() || "";
    const output = toonFieldValue(raw, "output");
    const exitCode = toonFieldValue(raw, "exitCode");
    const pid = toonFieldValue(raw, "pid");
    const lines: string[] = [];
    if (command) lines.push(`$ ${command}`);
    if (pid) lines.push(`pid ${pid}`);
    if (exitCode) lines.push(`exit ${exitCode}`);
    const body = output || error;
    if (body) lines.push(body);
    return lines.join("\n") || raw;
  }
  return error || raw;
};

const lspClass = (severity: string): string => {
  if (severity === "error") return "chat-tools__lsp-item chat-tools__lsp-item--error";
  if (severity === "warning") return "chat-tools__lsp-item chat-tools__lsp-item--warning";
  return "chat-tools__lsp-item";
};

const LspNotes = ({
  display,
  cleanLabel,
}: {
  display: ToolDisplay | undefined;
  cleanLabel: string;
}): ReactNode => {
  const notes = display?.diagnostics;
  if (!notes) return null;
  if (notes.length === 0) {
    return <p className="chat-tools__lsp chat-tools__lsp--clean">{cleanLabel}</p>;
  }
  return (
    <ul className="chat-tools__lsp">
      {notes.slice(0, 8).map((item, index) => (
        <li key={`${item.line}-${index}`} className={lspClass(item.severity)}>
          {item.severity} L{item.line}
          {item.path && item.path !== display?.path ? ` ${item.path}` : ""}: {item.message}
        </li>
      ))}
    </ul>
  );
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
  if (name === "bash" || name === "background") {
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
  const [image, setImage] = useState<{ titleKey: string; data: string } | null>(null);
  const [taskSessionId, setTaskSessionId] = useState<string | null>(null);
  const selection = useSelectionStore((state) => state.selection);
  const providers = useProvidersStore((state) => state.providers);
  const vision = useMemo(() => resolveVisionModel(providers, selection), [providers, selection]);
  if (calls.length === 0) return null;

  const openPreview = async (call: ChatToolCall): Promise<void> => {
    const display = call.display;
    if (call.name === "task" && display?.childSessionId) {
      setTaskSessionId(display.childSessionId);
      return;
    }
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
        const packed = (await runDiffLinesJob(before.content, after.content)).value;
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
    if (display?.imageData) {
      setImage({ titleKey, data: display.imageData });
      return;
    }
    const raw = call.output ?? "";
    const parsedRead =
      call.name === "read" && toonFieldValue(raw, "content") ? readToolView(raw) : null;
    setPreview({
      titleKey,
      value: parsedRead?.content ?? previewFromOutput(call),
      path: display?.path,
      startLine: parsedRead ? (parsedRead.startLine ?? display?.startLine) : undefined,
      language: call.name === "skill" ? "markdown" : undefined,
    });
  };

  return (
    <>
      <ul className="chat-tools">
        {calls.map((call, index) => {
          const display = call.display;
          const tokens = toolCallTokens(call, vision);
          const isAction = display?.kind === "action";
          const lineRange =
            display?.startLine !== undefined && display.endLine !== undefined
              ? t("chat.tools.lines", { start: display.startLine, end: display.endLine })
              : "";
          const label = toolCallLabel(call, lineRange);
          const canOpen =
            Boolean(call.output) ||
            Boolean(display?.imageData) ||
            Boolean(display?.childSessionId) ||
            Boolean(isAction && call.id);
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
                {tokens > 0 ? (
                  <span
                    className="chat-tools__tokens"
                    title={t("chat.tools.tokenHint", { count: tokens })}
                  >
                    ~{formatContextWindow(tokens)}
                  </span>
                ) : null}
              </span>
              <LspNotes display={display} cleanLabel={t("chat.tools.lspClean")} />
            </li>
          );
        })}
      </ul>
      <ReadOnlyEditorDialog
        open={preview !== null}
        titleKey={preview?.titleKey ?? "chat.tools.outputTitle"}
        value={preview?.value ?? ""}
        path={preview?.path}
        language={preview?.language}
        startLine={preview?.startLine}
        lineNumbers={preview?.lineNumbers}
        lineKinds={preview?.lineKinds}
        onOpenChange={(open) => {
          if (!open) setPreview(null);
        }}
      />
      <Dialog
        open={image !== null}
        onOpenChange={(open) => {
          if (!open) setImage(null);
        }}
        titleKey={image?.titleKey ?? "chat.tools.shotTitle"}
        size="wide"
      >
        {image ? (
          <img
            className="tool-result-dialog__image"
            alt=""
            src={`data:image/png;base64,${image.data}`}
          />
        ) : null}
      </Dialog>
      <TaskChatDialog
        key={taskSessionId ?? "closed"}
        sessionId={taskSessionId}
        onClose={() => setTaskSessionId(null)}
      />
    </>
  );
};

const TaskMessage = ({
  message,
  sessionId,
}: {
  message: ChatMessage;
  sessionId: string;
}): ReactNode => {
  const rounds = message.toolRounds ?? [];
  const lastRound = rounds[rounds.length - 1]?.content;
  const trailing = rounds.length > 0 && message.content !== (lastRound ?? "");
  return (
    <div className={`chat-message chat-message--${message.role}`}>
      {message.role === "user" ? (
        message.content ? (
          <p className="chat-message__content">{message.content}</p>
        ) : null
      ) : (
        <>
          {rounds.map((round, index) => (
            <div key={`round-${index}`}>
              <ChatMarkdown content={round.content ?? ""} />
              <ToolCallsBlock sessionId={sessionId} calls={round.calls ?? []} />
            </div>
          ))}
          {rounds.length === 0 || trailing ? <ChatMarkdown content={message.content} /> : null}
        </>
      )}
    </div>
  );
};

const TaskChatDialog = ({
  sessionId,
  onClose,
}: {
  sessionId: string | null;
  onClose: () => void;
}): ReactNode => {
  const { t } = useTranslation();
  const [session, setSession] = useState<SessionRecord | null>(null);
  const [missing, setMissing] = useState(false);
  useEffect(() => {
    if (!sessionId) return;
    let alive = true;
    const load = () => {
      void invoke<SessionRecord>("read_session", { sessionId })
        .then((next) => {
          if (!alive) return;
          setMissing(false);
          setSession(next);
        })
        .catch(() => {
          if (alive) setMissing(true);
        });
    };
    load();
    const timer = window.setInterval(load, 1000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [sessionId]);
  return (
    <Dialog
      open={sessionId !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      titleKey="chat.tools.taskChat"
      size="wide"
    >
      <div className="task-chat">
        {missing ? <p className="task-chat__status">{t("chat.tools.taskMissing")}</p> : null}
        {session?.messages.map((message) => (
          <TaskMessage key={message.id} message={message} sessionId={sessionId ?? ""} />
        ))}
      </div>
    </Dialog>
  );
};

export { ToolCallsBlock };
export type { ToolCallsBlockProps };
