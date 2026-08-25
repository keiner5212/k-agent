import { normalizeMentionPath } from "@/lib/file-mentions";

export type MentionEditResult = {
  next: string;
  cursor: number;
};

const SLASH_MENTION_RE = /\/[a-zA-Z][\w-]*\s+\([^)]+\)\s?/g;
const FILE_MENTION_RE = /@([^\s@]+)/g;

const trimFileMentionPath = (path: string): string | null => {
  if (!path) return null;
  const normalized = normalizeMentionPath(path);
  const hadTrailingSlash = normalized.endsWith("/");
  const core = hadTrailingSlash ? normalized.slice(0, -1) : normalized;
  const parts = core.split("/").filter(Boolean);
  if (parts.length === 0) return null;
  parts.pop();
  if (parts.length === 0) return "";
  const joined = parts.join("/");
  if (hadTrailingSlash || normalized.includes("/")) return `${joined}/`;
  return joined;
};

export const tryMentionBackspace = (text: string, cursor: number): MentionEditResult | null => {
  if (cursor <= 0) return null;

  for (const match of text.matchAll(SLASH_MENTION_RE)) {
    const start = match.index ?? 0;
    const end = start + (match[0]?.length ?? 0);
    if (cursor > start && cursor <= end) {
      return { next: text.slice(0, start) + text.slice(end), cursor: start };
    }
  }

  for (const match of text.matchAll(FILE_MENTION_RE)) {
    const start = match.index ?? 0;
    const end = start + (match[0]?.length ?? 0);
    if (cursor <= start || cursor > end) continue;
    const path = match[1] ?? "";
    const trimmed = trimFileMentionPath(path);
    if (trimmed === null) {
      return { next: text.slice(0, start) + text.slice(end), cursor: start };
    }
    const token = `@${trimmed}`;
    return {
      next: text.slice(0, start) + token + text.slice(end),
      cursor: start + token.length,
    };
  }

  return null;
};
