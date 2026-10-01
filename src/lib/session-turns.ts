import type { ChatMessage, ChatTurn, TodoDiff, TodoItem } from "@/types/chat";
import type {
  FileTouch,
  RedoRecord,
  SessionRecord,
  SessionsSnapshot,
  TodoHistoryEvent,
  TurnCheckpoint,
} from "@/types/sessions";

type LooseTodoItem = {
  id?: unknown;
  content?: unknown;
  status?: unknown;
  priority?: unknown;
};

type LooseTodoDiff = {
  added?: unknown;
  updated?: unknown;
  removed?: unknown;
  cleared?: unknown;
};

type LooseTodoHistoryEvent = {
  timestamp?: unknown;
  diff?: unknown;
};

type LooseSession = {
  id: string;
  title?: string;
  preview?: string;
  updatedAt?: number;
  messages?: ChatMessage[] | null;
  todos?: unknown;
  todosHistory?: unknown;
  outsideWorkspaceAllowed?: boolean;
  httpWriteAllowed?: boolean;
  workspacePath?: unknown;
  parentSessionId?: unknown;
  fileCheckpoints?: unknown;
  redo?: unknown;
};

export const sessionMessages = (
  session: { messages?: ChatMessage[] | null } | null | undefined,
): ChatMessage[] => (Array.isArray(session?.messages) ? session.messages : []);

const legacyPriorityToNumber = (value: unknown): number => {
  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.max(0, Math.min(10, Math.round(value)));
  }
  if (value === "high") return 9;
  if (value === "medium") return 5;
  if (value === "low") return 1;
  return 5;
};

const sanitizeTodoStatus = (value: unknown): TodoItem["status"] => {
  if (value === "in_progress" || value === "completed" || value === "cancelled") return value;
  return "pending";
};

const legacyContentHash = (content: string, seed: number): string => {
  let hash = 0x811c9dc5 ^ seed;
  for (let index = 0; index < content.length; index += 1) {
    hash ^= content.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
};

const sanitizeTodoItem = (value: unknown, index: number): TodoItem | null => {
  if (!value || typeof value !== "object") return null;
  const item = value as LooseTodoItem;
  const content = typeof item.content === "string" ? item.content.trim() : "";
  if (!content) return null;
  const idRaw = typeof item.id === "string" ? item.id.trim() : "";
  const id = idRaw.length > 0 ? idRaw : `legacy-${legacyContentHash(content, index)}`;
  return {
    id,
    content,
    status: sanitizeTodoStatus(item.status),
    priority: legacyPriorityToNumber(item.priority),
  };
};

const sanitizeTodoItems = (value: unknown): TodoItem[] => {
  if (!Array.isArray(value)) return [];
  const items: TodoItem[] = [];
  for (let index = 0; index < value.length; index += 1) {
    const item = sanitizeTodoItem(value[index], index);
    if (item) items.push(item);
  }
  return items;
};

const sanitizeTodoDiff = (value: unknown): TodoDiff => {
  if (!value || typeof value !== "object") return {};
  const diff = value as LooseTodoDiff;
  const result: TodoDiff = {};
  for (const [key, target] of [
    [diff.added, "added"],
    [diff.updated, "updated"],
  ] as const) {
    if (!Array.isArray(key)) continue;
    const items: TodoItem[] = [];
    for (let index = 0; index < key.length; index += 1) {
      const sanitized = sanitizeTodoItem(key[index], index);
      if (sanitized) items.push(sanitized);
    }
    if (items.length > 0 || key.length > 0) {
      result[target] = items;
    }
  }
  if (Array.isArray(diff.removed)) {
    result.removed = diff.removed.filter((entry): entry is string => typeof entry === "string");
  }
  if (typeof diff.cleared === "boolean") {
    result.cleared = diff.cleared;
  }
  return result;
};

const sanitizeTodoHistory = (value: unknown): TodoHistoryEvent[] => {
  if (!Array.isArray(value)) return [];
  const events: TodoHistoryEvent[] = [];
  for (const entry of value) {
    if (!entry || typeof entry !== "object") continue;
    const item = entry as LooseTodoHistoryEvent;
    if (typeof item.timestamp !== "number") continue;
    events.push({ timestamp: item.timestamp, diff: sanitizeTodoDiff(item.diff) });
  }
  return events;
};

const sanitizeFileTouch = (value: unknown): FileTouch | null => {
  if (!value || typeof value !== "object") return null;
  const item = value as Partial<FileTouch>;
  if (
    typeof item.path !== "string" ||
    typeof item.beforeHash !== "string" ||
    typeof item.afterHash !== "string"
  ) {
    return null;
  }
  if (!item.path || !item.beforeHash || !item.afterHash) return null;
  return {
    path: item.path,
    beforeHash: item.beforeHash,
    afterHash: item.afterHash,
  };
};

const sanitizeCheckpoints = (value: unknown): TurnCheckpoint[] => {
  if (!Array.isArray(value)) return [];
  const checkpoints: TurnCheckpoint[] = [];
  for (const entry of value) {
    if (!entry || typeof entry !== "object") continue;
    const item = entry as { turnId?: unknown; checkpointId?: unknown; files?: unknown };
    if (typeof item.turnId !== "string" || !item.turnId || !Array.isArray(item.files)) continue;
    if (typeof item.checkpointId !== "string" || !item.checkpointId) continue;
    const files = item.files
      .map(sanitizeFileTouch)
      .filter((file): file is FileTouch => file !== null);
    if (files.length === 0) continue;
    checkpoints.push({ turnId: item.turnId, checkpointId: item.checkpointId, files });
  }
  return checkpoints;
};

const sanitizeRedo = (value: unknown): RedoRecord | undefined => {
  if (!value || typeof value !== "object") return undefined;
  const item = value as { messages?: unknown; checkpoints?: unknown };
  if (!Array.isArray(item.messages)) return undefined;
  const messages = item.messages.filter(
    (message): message is ChatMessage =>
      Boolean(message) &&
      typeof message === "object" &&
      typeof (message as ChatMessage).id === "string" &&
      typeof (message as ChatMessage).role === "string" &&
      typeof (message as ChatMessage).content === "string",
  );
  return { messages, checkpoints: sanitizeCheckpoints(item.checkpoints) };
};

export const sanitizeSessionRecord = (session: LooseSession): SessionRecord => {
  const fileCheckpoints = sanitizeCheckpoints(session.fileCheckpoints);
  const redo = sanitizeRedo(session.redo);
  return {
    id: session.id,
    title: session.title ?? "",
    preview: session.preview ?? "",
    updatedAt: typeof session.updatedAt === "number" ? session.updatedAt : 0,
    messages: sessionMessages(session),
    todos: sanitizeTodoItems(session.todos),
    todosHistory: sanitizeTodoHistory(session.todosHistory),
    outsideWorkspaceAllowed: session.outsideWorkspaceAllowed === true,
    httpWriteAllowed: session.httpWriteAllowed === true,
    ...(typeof session.workspacePath === "string" && session.workspacePath.trim().length > 0
      ? { workspacePath: session.workspacePath }
      : {}),
    ...(typeof session.parentSessionId === "string" && session.parentSessionId.trim().length > 0
      ? { parentSessionId: session.parentSessionId.trim() }
      : {}),
    ...(fileCheckpoints.length > 0 ? { fileCheckpoints } : {}),
    ...(redo && redo.messages.length > 0 ? { redo } : {}),
  };
};

export const sanitizeSessionsSnapshot = (snapshot: {
  activeSessionId: string;
  sessions?: LooseSession[] | null;
}): SessionsSnapshot => ({
  activeSessionId: snapshot.activeSessionId,
  sessions: (snapshot.sessions ?? []).map(sanitizeSessionRecord),
});

const SUMMARY_TAIL = 2;
const SUMMARY_CHAR_BUDGET = 80_000;
const MESSAGE_CLIP = 4_000;

const clip = (text: string, max: number): string =>
  text.length <= max ? text : text.slice(text.length - max);

const messageText = (message: ChatMessage): string => {
  const parts: string[] = [];
  const push = (value: string | undefined): void => {
    const trimmed = value?.trim() ?? "";
    if (trimmed.length > 0) parts.push(clip(trimmed, MESSAGE_CLIP));
  };
  push(message.content);
  push(message.reasoning);
  for (const round of message.toolRounds ?? []) {
    push(round.content);
    for (const call of round.calls ?? []) {
      const args = call.arguments || call.argument || "";
      push(`${call.name} ${args}`.trim());
      push(call.output?.slice(0, 500));
    }
  }
  return parts.join("\n");
};

export const messagesBeforeTail = (
  messages: ChatMessage[],
): { head: ChatMessage[]; tail: ChatMessage[] } => {
  if (messages.length <= SUMMARY_TAIL) return { head: [], tail: messages };
  return {
    head: messages.slice(0, -SUMMARY_TAIL),
    tail: messages.slice(-SUMMARY_TAIL),
  };
};

export const summaryTranscript = (messages: ChatMessage[]): string => {
  const lines = messages
    .map((message) => {
      const body = messageText(message);
      return body.length > 0 ? `${message.role}: ${body}` : "";
    })
    .filter((line) => line.length > 0);
  if (lines.length === 0) return "";
  const first = lines[0] ?? "";
  let used = first.length;
  const recent: string[] = [];
  for (let index = lines.length - 1; index >= 1; index -= 1) {
    const line = lines[index] ?? "";
    if (used + line.length + 1 > SUMMARY_CHAR_BUDGET) break;
    recent.push(line);
    used += line.length + 1;
  }
  recent.reverse();
  return [first, ...recent].join("\n");
};

const REMINDER_OPEN = "<system-reminder>";
const REMINDER_CLOSE = "</system-reminder>";

const stripSystemReminders = (content: string): string => {
  let result = "";
  let index = 0;
  while (index < content.length) {
    const start = content.indexOf(REMINDER_OPEN, index);
    if (start === -1) {
      result += content.slice(index);
      break;
    }
    result += content.slice(index, start);
    let depth = 1;
    let cursor = start + REMINDER_OPEN.length;
    while (cursor < content.length && depth > 0) {
      const nextOpen = content.indexOf(REMINDER_OPEN, cursor);
      const nextClose = content.indexOf(REMINDER_CLOSE, cursor);
      if (nextClose === -1) {
        cursor = content.length;
        break;
      }
      if (nextOpen !== -1 && nextOpen < nextClose) {
        depth += 1;
        cursor = nextOpen + REMINDER_OPEN.length;
        continue;
      }
      depth -= 1;
      cursor = nextClose + REMINDER_CLOSE.length;
    }
    index = cursor;
  }
  return result
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
};

export const applySystemReminder = (
  turns: ChatTurn[],
  system: string,
  interval: number,
): ChatTurn[] => {
  const text = system.trim();
  if (text.length === 0 || interval <= 0) return turns;
  let userCount = 0;
  let lastUser = -1;
  for (let index = 0; index < turns.length; index += 1) {
    const turn = turns[index];
    if (!turn || turn.role !== "user" || turn.toolResult) continue;
    userCount += 1;
    lastUser = index;
  }
  if (userCount === 0 || userCount % interval !== 0 || lastUser < 0) return turns;
  const reminder = `<system-reminder>\n${text}\n</system-reminder>`;
  return turns.map((turn, index) => {
    const stripped = turn.content.includes(REMINDER_OPEN)
      ? stripSystemReminders(turn.content)
      : turn.content;
    if (index !== lastUser) {
      return stripped === turn.content ? turn : { ...turn, content: stripped };
    }
    return {
      ...turn,
      content: stripped.length > 0 ? `${stripped}\n\n${reminder}` : reminder,
    };
  });
};

export const toChatTurns = (messages: ChatMessage[]): ChatTurn[] => {
  const turns: ChatTurn[] = [];
  for (const message of messages) {
    if (message.streaming) continue;
    if (message.role === "assistant") {
      const rounds = message.toolRounds ?? [];
      const splitReasoning = rounds.length > 0;
      for (const round of rounds) {
        const calls = round.calls ?? [];
        turns.push({
          role: "assistant",
          content: round.content ?? "",
          reasoning: round.reasoning || null,
          reasoningSignature: round.reasoningSignature ?? null,
          attachments: message.attachments,
          toolCalls: calls.map((call) => ({
            id: call.id,
            name: call.name,
            argument: call.argument,
            arguments: call.arguments,
            thoughtSignature: call.thoughtSignature,
          })),
        });
        for (const call of calls) {
          if (!call.id || call.output === undefined) continue;
          turns.push({
            role: "user",
            content: "",
            toolResult: {
              callId: call.id,
              name: call.name,
              content: call.output,
              ...(call.display?.imageData ? { imageData: call.display.imageData } : {}),
            },
          });
        }
      }
      const body = message.shellAiSummary ?? message.content;
      if (
        body.trim().length > 0 ||
        (message.attachments?.length ?? 0) > 0 ||
        (!splitReasoning && rounds.length === 0 && (message.reasoning ?? "").trim().length > 0)
      ) {
        turns.push({
          role: "assistant",
          content: body,
          reasoning: splitReasoning || rounds.length === 0 ? (message.reasoning ?? null) : null,
          reasoningSignature:
            splitReasoning || rounds.length === 0 ? (message.reasoningSignature ?? null) : null,
          attachments: message.attachments,
        });
      }
      continue;
    }
    if (
      message.content.trim().length > 0 ||
      (message.reasoning ?? "").trim().length > 0 ||
      (message.attachments?.length ?? 0) > 0
    ) {
      turns.push({
        role: message.role,
        content: message.shellAiSummary ?? message.content,
        reasoning: message.reasoning ?? null,
        reasoningSignature: message.reasoningSignature ?? null,
        attachments: message.attachments,
      });
    }
  }
  return turns;
};
