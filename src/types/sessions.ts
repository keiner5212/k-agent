import type { ChatMessage } from "./chat";

export type SessionRecord = {
  id: string;
  title: string;
  preview: string;
  updatedAt: number;
  messages: ChatMessage[];
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
