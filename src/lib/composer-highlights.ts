import { fileMentionSpans, isExactMentionPath } from "@/lib/file-mentions";
import type { MentionSegment } from "@/lib/file-mentions";
import { ACTION_SLASH_NAMES } from "@/lib/slash-commands";

const SLASH_MENTION_RE = /\/([a-zA-Z][\w-]*)\s+\([^)]+\)/g;
const BARE_ACTION_RE = new RegExp(
  `(?:^|(?<=\\s))/(?:${ACTION_SLASH_NAMES.join("|")})(?![\\w-])`,
  "g",
);
export const segmentComposerHighlights = (
  text: string,
  paths: ReadonlySet<string>,
  slashNames: ReadonlySet<string>,
): MentionSegment[] => {
  if (!text.includes("@") && !text.includes("/")) return [{ text, mention: false }];

  const spans: Array<{ start: number; end: number }> = [];
  const mark = (start: number, end: number): void => {
    spans.push({ start, end });
  };

  for (const match of text.matchAll(SLASH_MENTION_RE)) {
    const name = match[1] ?? "";
    const index = match.index ?? 0;
    if (slashNames.has(name) || slashNames.has(name.toLowerCase())) {
      mark(index, index + (match[0]?.length ?? 0));
    }
  }

  BARE_ACTION_RE.lastIndex = 0;
  for (const match of text.matchAll(BARE_ACTION_RE)) {
    const index = match.index ?? 0;
    mark(index, index + (match[0]?.length ?? 0));
  }

  for (const mention of fileMentionSpans(text)) {
    const covered = spans.some((span) => mention.start >= span.start && mention.start < span.end);
    if (covered) continue;
    if (isExactMentionPath(paths, mention.path.replace(/\/+$/, ""))) {
      mark(mention.start, mention.end);
    }
  }

  if (spans.length === 0) return [{ text, mention: false }];

  spans.sort((a, b) => a.start - b.start);
  const merged: Array<{ start: number; end: number }> = [];
  for (const span of spans) {
    const last = merged[merged.length - 1];
    if (!last || span.start > last.end) {
      merged.push({ ...span });
      continue;
    }
    last.end = Math.max(last.end, span.end);
  }

  const segments: MentionSegment[] = [];
  let last = 0;
  for (const span of merged) {
    if (span.start > last) {
      segments.push({ text: text.slice(last, span.start), mention: false });
    }
    segments.push({ text: text.slice(span.start, span.end), mention: true });
    last = span.end;
  }
  if (last < text.length) {
    segments.push({ text: text.slice(last), mention: false });
  }
  return segments;
};
