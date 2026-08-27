import { perfLog } from "@/lib/perf-log";
import type { WorkspaceEntry } from "@/types/workspace-files";
import { joinWorkspacePath, toPosixPath } from "@/lib/path";

export type ActiveMention = {
  start: number;
  end: number;
  query: string;
};

export type MentionSegment = {
  text: string;
  mention: boolean;
};

export type MentionListingContext = {
  parentDir: string;
  prefix: string;
};

const MENTION_PATH_RE = /@([^\s@]+)/g;
export const MENTION_RESULT_LIMIT = 20;
export const ROOT_DIR = "";
const FILTER_LOG_MS = 8;

export type MentionFilterResult = {
  items: WorkspaceEntry[];
  tooMany: boolean;
};

export const parseActiveMention = (text: string, cursor: number): ActiveMention | null => {
  const before = text.slice(0, cursor);
  const atIndex = before.lastIndexOf("@");
  if (atIndex === -1) return null;
  if (atIndex > 0 && !/\s/.test(before.charAt(atIndex - 1))) return null;
  const query = before.slice(atIndex + 1);
  if (query.length > 0 && /\s/.test(query)) return null;
  return { start: atIndex, end: cursor, query };
};

export const mentionListingContext = (
  query: string,
  hasDir: (path: string) => boolean,
): MentionListingContext => {
  const normalized = toPosixPath(query);
  if (!normalized) return { parentDir: ROOT_DIR, prefix: "" };
  if (normalized.endsWith("/")) {
    return { parentDir: normalized.replace(/\/+$/, ""), prefix: "" };
  }
  const slash = normalized.lastIndexOf("/");
  if (slash === -1) {
    if (hasDir(normalized)) return { parentDir: normalized, prefix: "" };
    return { parentDir: ROOT_DIR, prefix: normalized };
  }
  return {
    parentDir: normalized.slice(0, slash),
    prefix: normalized.slice(slash + 1),
  };
};

export const dirsToLoadForMention = (
  query: string,
  hasDir: (path: string) => boolean,
): string[] => {
  const normalized = toPosixPath(query);
  const dirs = new Set<string>([ROOT_DIR]);
  if (!normalized) return [...dirs];

  if (normalized.endsWith("/")) {
    dirs.add(normalized.replace(/\/+$/, ""));
    return [...dirs];
  }

  const parts = normalized.split("/").filter(Boolean);
  let built = "";
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index];
    if (!part) continue;
    built = built ? `${built}/${part}` : part;
    const isLast = index === parts.length - 1;
    if (!isLast || hasDir(built)) dirs.add(built);
  }
  return [...dirs];
};

const entryName = (path: string): string => {
  const slash = path.lastIndexOf("/");
  return slash === -1 ? path : path.slice(slash + 1);
};

export const filterDirEntries = (
  entries: readonly WorkspaceEntry[],
  prefix: string,
): MentionFilterResult => {
  const start = performance.now();
  const lowerPrefix = prefix.toLowerCase();
  const items: WorkspaceEntry[] = [];
  for (const entry of entries) {
    if (lowerPrefix.length > 0 && !entryName(entry.path).toLowerCase().startsWith(lowerPrefix)) {
      continue;
    }
    items.push(entry);
    if (items.length > MENTION_RESULT_LIMIT) {
      perfLog("fileMentions.filter", performance.now() - start, {
        prefix,
        scanned: entries.length,
        tooMany: true,
      });
      return { items: [], tooMany: true };
    }
  }
  const elapsed = performance.now() - start;
  if (elapsed >= FILTER_LOG_MS) {
    perfLog("fileMentions.filter", elapsed, {
      prefix,
      scanned: entries.length,
      matches: items.length,
    });
  }
  return { items, tooMany: false };
};

export const buildMentionPathSet = (entries: readonly WorkspaceEntry[]): Set<string> => {
  const out = new Set<string>();
  for (const entry of entries) {
    out.add(entry.path);
    out.add(entry.path.toLowerCase());
  }
  return out;
};

export const isExactMentionPath = (paths: ReadonlySet<string>, rawPath: string): boolean => {
  const normalized = toPosixPath(rawPath);
  return paths.has(normalized) || paths.has(normalized.toLowerCase());
};

export const expandWorkspaceMentions = (
  text: string,
  entries: readonly WorkspaceEntry[],
  workspaceRoot: string,
  renderAbsolute: (absolute: string) => string,
): string => {
  if (!text.includes("@")) return text;
  const paths = buildMentionPathSet(entries);
  return text.replace(MENTION_PATH_RE, (match, raw: string) => {
    const normalized = toPosixPath(raw).replace(/\/+$/, "");
    if (!isExactMentionPath(paths, normalized)) return match;
    return renderAbsolute(joinWorkspacePath(workspaceRoot, normalized));
  });
};

export const segmentMentionHighlights = (
  text: string,
  paths: ReadonlySet<string>,
): MentionSegment[] => {
  if (!text.includes("@")) return [{ text, mention: false }];
  const segments: MentionSegment[] = [];
  let last = 0;
  for (const match of text.matchAll(MENTION_PATH_RE)) {
    const index = match.index ?? 0;
    if (index > last) {
      segments.push({ text: text.slice(last, index), mention: false });
    }
    const token = match[0] ?? "";
    const path = match[1] ?? "";
    segments.push({
      text: token,
      mention: isExactMentionPath(paths, path.replace(/\/+$/, "")),
    });
    last = index + token.length;
  }
  if (last < text.length) {
    segments.push({ text: text.slice(last), mention: false });
  }
  return segments;
};

export const applyMentionSelection = (
  text: string,
  mention: ActiveMention,
  entry: WorkspaceEntry,
): { next: string; cursor: number } => {
  const before = text.slice(0, mention.start);
  const after = text.slice(mention.end);
  const token = entry.kind === "dir" ? `${entry.path}/` : `${entry.path} `;
  const next = `${before}@${token}${after}`;
  const cursor = before.length + 1 + token.length;
  return { next, cursor };
};
