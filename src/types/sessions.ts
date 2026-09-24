import type { ChatMessage, TodoDiff, TodoItem } from "./chat";

export type TodoHistoryEvent = {
  timestamp: number;
  diff: TodoDiff;
};

export type SessionRecord = {
  id: string;
  title: string;
  preview: string;
  updatedAt: number;
  messages: ChatMessage[];
  todos?: TodoItem[];
  todosHistory?: TodoHistoryEvent[];
  outsideWorkspaceAllowed?: boolean;
  httpWriteAllowed?: boolean;
  workspacePath?: string;
};

export type SessionSummary = Pick<SessionRecord, "id" | "title" | "preview" | "updatedAt">;

export type SessionsSnapshot = {
  activeSessionId: string;
  sessions: SessionRecord[];
};

export const TITLE_MAX_LENGTH = 60;

export const titleFromFirstMessage = (text: string): string => {
  const line = text.trim().replace(/\s+/g, " ");
  if (line.length <= TITLE_MAX_LENGTH) return line;
  return `${line.slice(0, TITLE_MAX_LENGTH - 1)}...`;
};

export const toSessionSummary = (session: SessionRecord): SessionSummary => ({
  id: session.id,
  title: session.title,
  preview: session.preview,
  updatedAt: session.updatedAt,
});

export const sortSessions = (sessions: SessionRecord[]): SessionRecord[] =>
  [...sessions].sort((a, b) => b.updatedAt - a.updatedAt);

export const workspaceKey = (path: string | null | undefined): string => {
  if (!path) return "";
  return path.replace(/\\/g, "/").replace(/\/+$/, "");
};

export const sessionInWorkspace = (
  session: { workspacePath?: string },
  workspacePath: string | null | undefined,
): boolean => {
  const current = workspaceKey(workspacePath);
  const owned = workspaceKey(session.workspacePath);
  if (!current) return owned.length === 0;
  return owned === current;
};

export const adoptUnscopedSessions = (
  sessions: SessionRecord[],
  workspacePath: string | null | undefined,
): { sessions: SessionRecord[]; changed: boolean } => {
  const key = workspaceKey(workspacePath);
  if (!key) return { sessions, changed: false };
  let changed = false;
  const next = sessions.map((session) => {
    if (workspaceKey(session.workspacePath)) return session;
    changed = true;
    return { ...session, workspacePath: key };
  });
  return { sessions: changed ? next : sessions, changed };
};

export const activateWorkspace = (
  sessions: SessionRecord[],
  activeSessionId: string | null,
  workspacePath: string | null | undefined,
  createId: () => string,
): { sessions: SessionRecord[]; activeSessionId: string; changed: boolean } => {
  const key = workspaceKey(workspacePath);
  const current = activeSessionId
    ? sessions.find((session) => session.id === activeSessionId)
    : undefined;
  if (!key) {
    if (current) return { sessions, activeSessionId: current.id, changed: false };
    const session: SessionRecord = {
      id: createId(),
      title: "",
      preview: "",
      updatedAt: Date.now(),
      messages: [],
    };
    return {
      sessions: sortSessions([session, ...sessions]),
      activeSessionId: session.id,
      changed: true,
    };
  }
  if (current && sessionInWorkspace(current, key)) {
    return { sessions, activeSessionId: current.id, changed: false };
  }
  const newest = sortSessions(sessions.filter((session) => sessionInWorkspace(session, key)))[0];
  if (newest) return { sessions, activeSessionId: newest.id, changed: true };
  const session: SessionRecord = {
    id: createId(),
    title: "",
    preview: "",
    updatedAt: Date.now(),
    messages: [],
    workspacePath: key,
  };
  return {
    sessions: sortSessions([session, ...sessions]),
    activeSessionId: session.id,
    changed: true,
  };
};
