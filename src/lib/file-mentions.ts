import type { WorkspaceEntry } from "@/types/workspace-files";
import { perfLog } from "@/lib/perf-log";

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
const ROOT_DIR = "";

export type MentionFilterResult = {
  items: WorkspaceEntry[];
  tooMany: boolean;
};

export const normalizeMentionPath = (value: string): string => value.replace(/\\/g, "/");

export const parseActiveMention = (text: string, cursor: number): ActiveMention | null => {
  const before = text.slice(0, cursor);
  const atIndex = before.lastIndexOf("@");
  if (atIndex === -1) return null;
  if (atIndex > 0 && !/\s/.test(before.charAt(atIndex - 1))) return null;
  const query = before.slice(atIndex + 1);
  if (query.length > 0 && /\s/.test(query)) return null;
  return { start: atIndex, end: cursor, query };
};

const isDirPath = (entries: readonly WorkspaceEntry[], path: string): boolean => {
  const lower = path.toLowerCase();
  return entries.some((entry) => entry.kind === "dir" && entry.path.toLowerCase() === lower);
};

export const mentionListingContext = (
  query: string,
  entries: readonly WorkspaceEntry[],
): MentionListingContext => {
  const normalized = normalizeMentionPath(query);
  if (!normalized) {
    return { parentDir: ROOT_DIR, prefix: "" };
  }
  if (normalized.endsWith("/")) {
    return { parentDir: normalized.replace(/\/+$/, ""), prefix: "" };
  }
  const slash = normalized.lastIndexOf("/");
  if (slash === -1) {
    if (isDirPath(entries, normalized)) {
      return { parentDir: normalized, prefix: "" };
    }
    return { parentDir: ROOT_DIR, prefix: normalized };
  }
  return {
    parentDir: normalized.slice(0, slash),
    prefix: normalized.slice(slash + 1),
  };
};

export const dirsToLoadForMention = (
  query: string,
  entries: readonly WorkspaceEntry[],
): string[] => {
  const normalized = normalizeMentionPath(query);
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
    if (!isLast) {
      dirs.add(built);
      continue;
    }
    if (isDirPath(entries, built)) {
      dirs.add(built);
    }
  }
  return [...dirs];
};

const scopeEntries = (
  entries: readonly WorkspaceEntry[],
  parentDir: string,
  prefix: string,
): WorkspaceEntry[] => {
  const lowerPrefix = prefix.toLowerCase();
  const scoped: WorkspaceEntry[] = [];
  for (const entry of entries) {
    if (parentDir === ROOT_DIR) {
      const slash = entry.path.indexOf("/");
      if (slash !== -1) continue;
      if (prefix.length > 0 && !entry.path.toLowerCase().startsWith(lowerPrefix)) continue;
      scoped.push(entry);
      continue;
    }
    const parentPrefix = `${parentDir}/`;
    if (!entry.path.startsWith(parentPrefix)) continue;
    const rest = entry.path.slice(parentPrefix.length);
    if (!rest || rest.includes("/")) continue;
    if (prefix.length > 0 && !rest.toLowerCase().startsWith(lowerPrefix)) continue;
    scoped.push(entry);
  }
  return scoped;
};

export const filterWorkspaceEntries = (
  entries: readonly WorkspaceEntry[],
  query: string,
): MentionFilterResult => {
  const start = performance.now();
  const context = mentionListingContext(query, entries);
  const scoped = scopeEntries(entries, context.parentDir, context.prefix);
  const sorted = [...scoped].sort((a, b) => a.path.localeCompare(b.path));
  const result: MentionFilterResult =
    sorted.length > MENTION_RESULT_LIMIT
      ? { items: [], tooMany: true }
      : { items: sorted, tooMany: false };

  perfLog("fileMentions.filter", performance.now() - start, {
    query: normalizeMentionPath(query),
    parentDir: context.parentDir || ".",
    prefix: context.prefix,
    entries: entries.length,
    scoped: scoped.length,
    matches: result.items.length,
    tooMany: result.tooMany,
  });
  return result;
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
  const normalized = normalizeMentionPath(rawPath);
  return paths.has(normalized) || paths.has(normalized.toLowerCase());
};

export const segmentMentionHighlights = (
  text: string,
  paths: ReadonlySet<string>,
): MentionSegment[] => {
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
      mention: isExactMentionPath(paths, path),
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
