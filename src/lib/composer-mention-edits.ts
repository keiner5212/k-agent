import { fileMentionSpans, formatMentionToken } from "@/lib/file-mentions";
import { toPosixPath } from "@/lib/path";
import { ACTION_SLASH_NAMES } from "@/lib/slash-commands";

export type MentionEditResult = {
  next: string;
  cursor: number;
};

const SLASH_MENTION_RE = /\/[a-zA-Z][\w-]*\s+\([^)]+\)\s?/g;
const BARE_ACTION_RE = new RegExp(
  `(?:^|(?<=\\s))/(?:${ACTION_SLASH_NAMES.join("|")})(?![\\w-])\\s?`,
  "g",
);
const trimFileMentionPath = (path: string): string | null => {
  if (!path) return null;
  const normalized = toPosixPath(path);
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

  BARE_ACTION_RE.lastIndex = 0;
  for (const match of text.matchAll(BARE_ACTION_RE)) {
    const start = match.index ?? 0;
    const end = start + (match[0]?.length ?? 0);
    if (cursor > start && cursor <= end) {
      return { next: text.slice(0, start) + text.slice(end), cursor: start };
    }
  }

  for (const mention of fileMentionSpans(text)) {
    if (cursor <= mention.start || cursor > mention.end) continue;
    const trimmed = trimFileMentionPath(mention.path);
    if (trimmed === null) {
      return {
        next: text.slice(0, mention.start) + text.slice(mention.end),
        cursor: mention.start,
      };
    }
    const token = formatMentionToken(trimmed);
    return {
      next: text.slice(0, mention.start) + token + text.slice(mention.end),
      cursor: mention.start + token.length,
    };
  }

  return null;
};
