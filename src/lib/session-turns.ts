import type { ChatMessage, ChatTurn, TodoDiff, TodoItem } from "@/types/chat";
import type { SessionRecord, SessionsSnapshot, TodoHistoryEvent } from "@/types/sessions";

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

export const sanitizeSessionRecord = (session: LooseSession): SessionRecord => ({
  id: session.id,
  title: session.title ?? "",
  preview: session.preview ?? "",
  updatedAt: typeof session.updatedAt === "number" ? session.updatedAt : 0,
  messages: sessionMessages(session),
  todos: sanitizeTodoItems(session.todos),
  todosHistory: sanitizeTodoHistory(session.todosHistory),
  outsideWorkspaceAllowed: session.outsideWorkspaceAllowed === true,
  httpWriteAllowed: session.httpWriteAllowed === true,
});

export const sanitizeSessionsSnapshot = (snapshot: {
  activeSessionId: string;
  sessions?: LooseSession[] | null;
}): SessionsSnapshot => ({
  activeSessionId: snapshot.activeSessionId,
  sessions: (snapshot.sessions ?? []).map(sanitizeSessionRecord),
});

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
