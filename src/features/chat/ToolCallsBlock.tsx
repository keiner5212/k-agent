import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import {
  BookOpen,
  Braces,
  Camera,
  CircleQuestionMark,
  FileDiff,
  FilePen,
  FolderPlus,
  FolderSearch,
  Globe,
  Link,
  ListTodo,
  MessagesSquare,
  ScrollText,
  Search,
  SquarePen,
  SquareTerminal,
  Terminal,
  Trash2,
  Wrench,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import { Dialog } from "@/components/Dialog";
import type { LineKind } from "@/components/LineEditor";
import { ReadOnlyEditorDialog } from "@/features/chat/ReadOnlyEditorDialog";
import { resolveVisionModel, toolCallTokens } from "@/lib/context-usage";
import { runDiffLinesJob } from "@/lib/jobs";
import { mcpToolRefs, useMcpServersStore, type McpToolRef } from "@/lib/mcp-servers";
import { useProvidersStore } from "@/lib/providers";
import { readSessionFileRevision, toonFieldValue } from "@/lib/session-files";
import { useSelectionStore } from "@/lib/selected-model";
import { AGENT_TOOL_IDS, type AgentToolId } from "@/types/agents";
import { type ChatToolCall, type ToolDisplay } from "@/types/chat";
import type { SessionRecord } from "@/types/sessions";
import { formatContextWindow } from "@/types/providers";
import { ChatTranscript } from "./ChatTranscript";
import { assistantIsAnswering, assistantIsThinking, ChatWaitingLine } from "./ChatWaitingLine";

type ToolCallsBlockProps = {
  calls: ChatToolCall[];
  sessionId: string | null;
  pending?: boolean;
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

const TOOL_TITLE: Record<string, string> = {
  skill: "chat.tools.skillTitle",
  read: "chat.tools.readTitle",
  glob: "chat.tools.globTitle",
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
  page_shot: "chat.tools.shotTitle",
  ask_user: "chat.tools.askTitle",
  apply_patch: "chat.tools.patchTitle",
  lsp: "chat.tools.lspTitle",
  task: "chat.tools.taskTitle",
  diagram: "chat.tools.diagramTitle",
};

const TOOL_ICON: Record<AgentToolId, LucideIcon> = {
  skill: BookOpen,
  read: ScrollText,
  write: FilePen,
  edit: SquarePen,
  glob: FolderSearch,
  ask_user: CircleQuestionMark,
  create_folder: FolderPlus,
  delete: Trash2,
  fetch_url: Link,
  internet_search: Globe,
  page_shot: Camera,
  todowrite: ListTodo,
  bash: Terminal,
  background: SquareTerminal,
  grep: Search,
  apply_patch: FileDiff,
  lsp: Braces,
  task: MessagesSquare,
  diagram: Workflow,
};

const isKnownTool = (name: string): name is AgentToolId =>
  (AGENT_TOOL_IDS as readonly string[]).includes(name);

const ToolIcon = ({ name }: { name: string }): ReactNode => {
  const Icon = isKnownTool(name) ? TOOL_ICON[name] : Wrench;
  return <Icon size={14} strokeWidth={1.75} />;
};

const toolDisplayName = (name: string, refs: Map<string, McpToolRef>): string => {
  const ref = refs.get(name);
  if (ref) return `${ref.serverName} / ${ref.toolName}`;
  return name;
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

const callArgs = (call: ChatToolCall): Record<string, unknown> | null => {
  for (const raw of [call.arguments, call.argument]) {
    const trimmed = raw?.trim();
    if (!trimmed || !trimmed.startsWith("{")) continue;
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>;
      }
    } catch {
      // Not a JSON object. Try the next candidate.
    }
  }
  return null;
};

const argText = (args: Record<string, unknown> | null, key: string): string => {
  const value = args?.[key];
  return typeof value === "string" ? value.trim() : "";
};

const argString = (call: ChatToolCall, key: string): string => argText(callArgs(call), key);

const legacyArgText = (call: ChatToolCall): string => {
  const raw = call.argument?.trim() ?? "";
  if (!raw || raw.startsWith("{")) return "";
  const cut = raw.indexOf(" :: ");
  return (cut > 0 ? raw.slice(0, cut) : raw).trim();
};

const fieldOr = (raw: string, key: string, fallback: string): string => {
  const value = toonFieldValue(raw, key);
  return value.length > 0 ? value : fallback;
};

const previewFromOutput = (call: ChatToolCall): string => {
  const raw = call.output ?? "";
  const error = toonFieldValue(raw, "error");
  const name = call.name;
  if (name === "skill") return fieldOr(raw, "body", error || raw);
  if (name === "glob") return fieldOr(raw, "entries", error || raw);
  if (name === "grep") return fieldOr(raw, "matches", error || raw);
  if (name === "internet_search") return fieldOr(raw, "items", error || raw);
  if (name === "fetch_url") {
    if (error) return error;
    const title = toonFieldValue(raw, "title");
    const content = toonFieldValue(raw, "content");
    if (title && content) return `${title}\n\n${content}`;
    return content || raw;
  }
  if (name === "ask_user") return fieldOr(raw, "answers", toonFieldValue(raw, "status") || raw);
  if (name === "page_shot") return error || toonFieldValue(raw, "url") || raw;
  if (name === "todowrite") {
    const todos = call.display?.todos;
    if (todos && todos.length > 0) {
      return todos.map((item) => `${item.status}  ${item.content}`).join("\n");
    }
    return fieldOr(raw, "summary", error || raw);
  }
  if (name === "bash" || name === "background") {
    const command = argString(call, "command") || call.argument?.trim() || "";
    const output = toonFieldValue(raw, "output");
    const exitCode = toonFieldValue(raw, "exitCode");
    const pid = toonFieldValue(raw, "pid");
    const lines: string[] = [];
    if (command) lines.push(`$ ${command}`);
    if (name === "bash") {
      const description = argString(call, "description");
      if (description) lines.push(description);
    }
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

const SUMMARY_MAX = 120;
const PATCH_FILE_RE = /^\*\*\* (?:Add|Update|Delete) File: (.+)$/m;

const TOOL_SUMMARY_KEYS: Record<string, readonly string[]> = {
  skill: ["name"],
  read: ["filePath"],
  write: ["filePath"],
  edit: ["filePath"],
  apply_patch: [],
  glob: ["pattern", "dirPath"],
  ask_user: [],
  create_folder: ["dirPath"],
  delete: ["path"],
  todowrite: [],
  bash: ["command"],
  background: ["command"],
  grep: ["pattern"],
  fetch_url: ["url"],
  internet_search: ["query"],
  page_shot: ["url"],
  lsp: ["filePath"],
  task: ["description"],
  diagram: ["brief"],
};

const toolSummary = (call: ChatToolCall): string => {
  const args = callArgs(call);
  for (const key of TOOL_SUMMARY_KEYS[call.name] ?? []) {
    const value = argText(args, key);
    if (value) return value;
  }
  if (call.name === "edit") {
    const rawEdits = args?.edits;
    const edits: unknown[] = Array.isArray(rawEdits) ? (rawEdits as unknown[]) : [];
    const first = edits[0];
    if (first && typeof first === "object") {
      const path = argText(first as Record<string, unknown>, "filePath");
      if (path) return path;
    }
  }
  if (call.name === "apply_patch") {
    const match = PATCH_FILE_RE.exec(argText(args, "patchText"));
    if (match) return match[1].trim();
  }
  const legacy = legacyArgText(call);
  if (legacy && legacy.length <= SUMMARY_MAX) return legacy;
  return call.display?.path?.trim() ?? "";
};

const summaryText = (value: string): string =>
  value.length > SUMMARY_MAX ? `${value.slice(0, SUMMARY_MAX - 3)}...` : value;

const callIsRunning = (call: ChatToolCall, pending: boolean | undefined): boolean => {
  if (!pending) return false;
  if (call.display?.status) return false;
  return (call.output?.length ?? 0) === 0;
};

const ToolCallsBlock = ({ calls, sessionId, pending }: ToolCallsBlockProps): ReactNode => {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<PreviewState | null>(null);
  const [image, setImage] = useState<{ titleKey: string; data: string } | null>(null);
  const [taskSessionId, setTaskSessionId] = useState<string | null>(null);
  const selection = useSelectionStore((state) => state.selection);
  const providers = useProvidersStore((state) => state.providers);
  const vision = useMemo(() => resolveVisionModel(providers, selection), [providers, selection]);
  const mcpServers = useMcpServersStore((state) => state.servers);
  const mcpTools = useMemo(() => mcpToolRefs(mcpServers), [mcpServers]);
  if (calls.length === 0) return null;

  const openPreview = async (call: ChatToolCall): Promise<void> => {
    const display = call.display;
    if (call.name === "task" && display?.childSessionId) {
      setTaskSessionId(display.childSessionId);
      return;
    }
    const fileEdit = call.name === "write" || call.name === "edit" || call.name === "apply_patch";
    if (fileEdit && display?.kind === "action" && call.id && sessionId) {
      const titleKey =
        call.name === "write"
          ? "chat.tools.writeTitle"
          : call.name === "apply_patch"
            ? "chat.tools.patchTitle"
            : "chat.tools.editTitle";
      if (display.status !== "ok") {
        setPreview({
          titleKey,
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
        if (call.name === "apply_patch" && !before.content && !after.content) {
          setPreview({
            titleKey,
            value: previewFromOutput(call),
            path: display.path,
          });
          return;
        }
        const packed = (await runDiffLinesJob(before.content, after.content)).value;
        setPreview({
          titleKey,
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
          const toolName = toolDisplayName(call.name, mcpTools);
          const summary = toolSummary(call);
          const bashDescription = call.name === "bash" ? argString(call, "description") : "";
          const runningCall = callIsRunning(call, pending);
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
                <span
                  className={
                    runningCall
                      ? "chat-tools__symbol chat-tools__symbol--running"
                      : display?.status === "error"
                        ? "chat-tools__symbol chat-tools__symbol--error"
                        : "chat-tools__symbol"
                  }
                  aria-hidden="true"
                >
                  <ToolIcon name={call.name} />
                </span>
                <span className="chat-tools__label">
                  {canOpen ? (
                    <button
                      type="button"
                      className={
                        runningCall ? "chat-tools__name chat-live-label" : "chat-tools__name"
                      }
                      onClick={() => {
                        void openPreview(call);
                      }}
                    >
                      {toolName}
                    </button>
                  ) : (
                    <span
                      className={
                        runningCall
                          ? "chat-tools__label-name chat-live-label"
                          : "chat-tools__label-name"
                      }
                    >
                      {toolName}
                    </span>
                  )}
                  {summary ? (
                    <span className="chat-tools__summary" title={summary}>
                      {summaryText(summary)}
                    </span>
                  ) : null}
                  {lineRange ? <span className="chat-tools__range">{lineRange}</span> : null}
                </span>
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
              {bashDescription ? (
                <span className="chat-tools__detail">{bashDescription}</span>
              ) : null}
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
        live={calls.some(
          (call) =>
            call.name === "task" &&
            call.display?.childSessionId === taskSessionId &&
            callIsRunning(call, pending),
        )}
        onClose={() => setTaskSessionId(null)}
      />
    </>
  );
};

const taskTranscriptStamp = (session: SessionRecord): string => {
  const parts: string[] = [];
  for (const message of session.messages) {
    parts.push(String(message.content.length), String(message.reasoning?.length ?? 0));
    for (const round of message.toolRounds ?? []) {
      parts.push(
        String(round.reasoning.length),
        String(round.content?.length ?? 0),
        String(round.calls.length),
        String(round.thinkingMs ?? ""),
      );
      for (const call of round.calls) {
        parts.push(call.id ?? "", String(call.output?.length ?? 0), call.display?.status ?? "");
      }
    }
  }
  return parts.join(":");
};

const showChildReasoning = (messages: SessionRecord["messages"]): SessionRecord["messages"] =>
  messages.map((message) => {
    if (message.role !== "assistant") return message;
    const reasoning = message.reasoning?.trim() ?? "";
    const rounds = message.toolRounds ?? [];
    if (reasoning.length === 0 || rounds.length === 0) return message;
    if (rounds.some((round) => round.reasoning.trim().length > 0)) return message;
    const last = rounds[rounds.length - 1];
    if (!last) return message;
    const next = rounds.slice();
    next[next.length - 1] = {
      ...last,
      reasoning,
      thinkingMs: last.thinkingMs ?? message.thinkingMs,
    };
    return { ...message, toolRounds: next };
  });

const liveChildMessages = (
  messages: SessionRecord["messages"],
  running: boolean,
): SessionRecord["messages"] => {
  if (!running || messages.length === 0) return messages;
  const lastIndex = messages.length - 1;
  const last = messages[lastIndex];
  if (!last || last.role !== "assistant") return messages;
  return messages.map((message, index) =>
    index === lastIndex ? { ...message, streaming: true } : message,
  );
};

const TaskChatDialog = ({
  sessionId,
  live,
  onClose,
}: {
  sessionId: string | null;
  live: boolean;
  onClose: () => void;
}): ReactNode => {
  const { t } = useTranslation();
  const [session, setSession] = useState<SessionRecord | null>(null);
  const [missing, setMissing] = useState(false);
  const messages = useMemo(
    () => liveChildMessages(showChildReasoning(session?.messages ?? []), live),
    [live, session],
  );
  const waiting =
    live &&
    !messages.some((message) => assistantIsAnswering(message) || assistantIsThinking(message));
  useEffect(() => {
    if (!sessionId) return;
    let alive = true;
    let seen = false;
    let timer = 0;
    let delay = 100;
    let quiet = 0;
    let stamp = "";
    const schedule = (): void => {
      timer = window.setTimeout(load, delay);
    };
    const load = (): void => {
      void invoke<SessionRecord>("read_session", { sessionId })
        .then((next) => {
          if (!alive) return;
          seen = true;
          setMissing(false);
          const nextStamp = taskTranscriptStamp(next);
          if (nextStamp !== stamp) {
            stamp = nextStamp;
            quiet = 0;
            delay = 100;
            setSession(next);
          } else {
            quiet += 1;
            if (quiet >= 5) delay = 1000;
          }
          schedule();
        })
        .catch(() => {
          if (!alive) return;
          if (!seen) setMissing(true);
          delay = 1000;
          schedule();
        });
    };
    load();
    return () => {
      alive = false;
      window.clearTimeout(timer);
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
      <div className="chat-thread-host">
        {missing ? <p className="task-chat__status">{t("chat.tools.taskMissing")}</p> : null}
        <ChatTranscript messages={messages} sessionId={sessionId} actions={false} peer="parent">
          <div className="chat-thread__reserve">{waiting ? <ChatWaitingLine /> : null}</div>
        </ChatTranscript>
      </div>
    </Dialog>
  );
};

export { ToolCallsBlock };
export type { ToolCallsBlockProps };
