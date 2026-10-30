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

export const MENTION_RESULT_LIMIT = 20;

export type FileMentionSpan = {
  start: number;
  end: number;
  path: string;
};

const fileMentionPattern = (): RegExp => /@(?:"((?:\\.|[^"\\])*)"|([^\s@]+))/g;

const unescapeMentionPath = (value: string): string => value.replace(/\\(["\\])/g, "$1");

const quoteIsOpen = (body: string): boolean => {
  let escaped = false;
  for (const char of body) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (char === "\\") {
      escaped = true;
      continue;
    }
    if (char === '"') return false;
  }
  return true;
};

export const fileMentionSpans = (text: string): FileMentionSpan[] => {
  const spans: FileMentionSpan[] = [];
  for (const match of text.matchAll(fileMentionPattern())) {
    const quoted = match[1];
    const bare = match[2];
    const path = quoted !== undefined ? unescapeMentionPath(quoted) : (bare ?? "");
    const start = match.index ?? 0;
    spans.push({ start, end: start + (match[0]?.length ?? 0), path });
  }
  return spans;
};

export const formatMentionToken = (path: string): string => {
  if (!/[\s"\\@]/.test(path)) return `@${path}`;
  const escaped = path.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  return `@"${escaped}"`;
};
export const ROOT_DIR = "";

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
  if (query.startsWith('"')) {
    const body = query.slice(1);
    if (!quoteIsOpen(body)) return null;
    return { start: atIndex, end: cursor, query: unescapeMentionPath(body) };
  }
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
  const lowerPrefix = prefix.toLowerCase();
  const items: WorkspaceEntry[] = [];
  for (const entry of entries) {
    if (lowerPrefix.length > 0 && !entryName(entry.path).toLowerCase().includes(lowerPrefix)) {
      continue;
    }
    items.push(entry);
    if (items.length > MENTION_RESULT_LIMIT) return { items: [], tooMany: true };
  }
  return { items, tooMany: false };
};

export const mentionSearchQuery = (query: string): string | null => {
  const normalized = toPosixPath(query).replace(/^\/+/, "");
  if (!normalized || normalized.endsWith("/") || normalized.includes("..")) return null;
  return normalized.replace(/\/+$/, "");
};

export const mergeMentionEntries = (
  scoped: readonly WorkspaceEntry[],
  prefix: string,
  hits: readonly WorkspaceEntry[],
): MentionFilterResult => {
  if (prefix.length === 0 || hits.length === 0) return filterDirEntries(scoped, prefix);
  const local = filterDirEntries(scoped, prefix);
  const items: WorkspaceEntry[] = [];
  const seen = new Set<string>();
  const push = (entry: WorkspaceEntry): void => {
    if (seen.has(entry.path) || items.length >= MENTION_RESULT_LIMIT) return;
    seen.add(entry.path);
    items.push(entry);
  };
  if (!local.tooMany) {
    for (const entry of local.items) push(entry);
  }
  for (const entry of hits) push(entry);
  return { items, tooMany: items.length === 0 && local.tooMany };
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
  let out = "";
  let last = 0;
  for (const span of fileMentionSpans(text)) {
    out += text.slice(last, span.start);
    const normalized = toPosixPath(span.path).replace(/\/+$/, "");
    out += isExactMentionPath(paths, normalized)
      ? renderAbsolute(joinWorkspacePath(workspaceRoot, normalized))
      : text.slice(span.start, span.end);
    last = span.end;
  }
  return out + text.slice(last);
};

export const segmentMentionHighlights = (
  text: string,
  paths: ReadonlySet<string>,
): MentionSegment[] => {
  if (!text.includes("@")) return [{ text, mention: false }];
  const segments: MentionSegment[] = [];
  let last = 0;
  for (const span of fileMentionSpans(text)) {
    if (span.start > last) {
      segments.push({ text: text.slice(last, span.start), mention: false });
    }
    segments.push({
      text: text.slice(span.start, span.end),
      mention: isExactMentionPath(paths, span.path.replace(/\/+$/, "")),
    });
    last = span.end;
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
  const path = entry.kind === "dir" ? `${entry.path}/` : entry.path;
  const token = formatMentionToken(path);
  const spacer = entry.kind === "dir" ? "" : " ";
  const next = `${before}${token}${spacer}${after}`;
  const cursor = before.length + token.length + spacer.length;
  return { next, cursor };
};
