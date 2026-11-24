import { invoke } from "@tauri-apps/api/core";
import type { ChatAttachment, ChatMessage, ToolDisplay } from "@/types/chat";

export type SessionChangedFile = {
  path: string;
  added: number;
  removed: number;
  callId: string;
  firstCallId: string;
  deleted: boolean;
};

const escapeRegex = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const decodeToonEscape = (raw: string): string =>
  raw
    .replace(/\\n/g, "\n")
    .replace(/\\r/g, "\r")
    .replace(/\\t/g, "\t")
    .replace(/\\"/g, '"')
    .replace(/\\\\/g, "\\");

/**
 * Extract the value of a top-level field from a TOON object body.
 * Handles double-quoted strings (with `\n`, `\"`, `\\` escapes),
 * single-quoted strings (no escapes), and bare scalars. Empty string
 * when the field is missing.
 */
export const toonFieldValue = (source: string, key: string): string => {
  const re = new RegExp(`(?:^|\\n)${escapeRegex(key)}:\\s*(.*)`);
  const match = re.exec(source);
  if (!match) return "";
  const raw = match[1];
  const trimmed = raw.replace(/\s+$/, "");
  if (trimmed.startsWith('"') && trimmed.endsWith('"') && trimmed.length >= 2) {
    return decodeToonEscape(trimmed.slice(1, -1));
  }
  if (trimmed.startsWith("'") && trimmed.endsWith("'") && trimmed.length >= 2) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
};

export { diffEditorValue, type DiffEditorValue, type DiffLineKind } from "./session-diff";

export const readSessionAttachment = (sessionId: string, attachmentId: string) =>
  invoke<ChatAttachment>("read_session_attachment", {
    input: { sessionId, attachmentId },
  });

export const readSessionFileRevision = (
  sessionId: string,
  callId: string,
  side: "before" | "after",
) =>
  invoke<{ content: string }>("read_session_file_revision", {
    input: { sessionId, callId, side },
  });

const isActionOk = (display: ToolDisplay | undefined): display is ToolDisplay =>
  display?.kind === "action" && display.status === "ok" && Boolean(display.path);

type PathState = {
  added: number;
  removed: number;
  callId: string;
  firstCallId: string;
  deleted: boolean;
};

export const netLineChange = (
  before: string,
  after: string,
): { added: number; removed: number } => {
  if (before === after) return { added: 0, removed: 0 };
  if (before.length === 0) return { added: lineCount(after), removed: 0 };
  if (after.length === 0) return { added: 0, removed: lineCount(before) };
  const oldLines = before.split("\n");
  const newLines = after.split("\n");
  if (oldLines.length * newLines.length > 4_000_000) {
    return spanLineChange(oldLines, newLines);
  }
  const shared = lcsLength(oldLines, newLines);
  return { added: newLines.length - shared, removed: oldLines.length - shared };
};

const lineCount = (text: string): number => (text.length === 0 ? 0 : text.split("\n").length);

const lcsLength = (left: string[], right: string[]): number => {
  const [short, long] = left.length <= right.length ? [left, right] : [right, left];
  let prev = new Array<number>(short.length + 1).fill(0);
  let curr = new Array<number>(short.length + 1).fill(0);
  for (const line of long) {
    for (let index = 0; index < short.length; index += 1) {
      const other = short[index];
      curr[index + 1] = line === other ? prev[index] + 1 : Math.max(curr[index], prev[index + 1]);
    }
    const swap = prev;
    prev = curr;
    curr = swap;
    curr.fill(0);
  }
  return prev[short.length] ?? 0;
};

const spanLineChange = (
  oldLines: string[],
  newLines: string[],
): { added: number; removed: number } => {
  let start = 0;
  while (
    start < oldLines.length &&
    start < newLines.length &&
    oldLines[start] === newLines[start]
  ) {
    start += 1;
  }
  let oldEnd = oldLines.length;
  let newEnd = newLines.length;
  while (oldEnd > start && newEnd > start && oldLines[oldEnd - 1] === newLines[newEnd - 1]) {
    oldEnd -= 1;
    newEnd -= 1;
  }
  return { added: newEnd - start, removed: oldEnd - start };
};

export const collectSessionChanges = (messages: ChatMessage[]): SessionChangedFile[] => {
  const byPath = new Map<string, PathState>();
  for (const message of messages) {
    for (const round of message.toolRounds ?? []) {
      for (const call of round.calls ?? []) {
        if (!isActionOk(call.display) || !call.display.path || !call.id) continue;
        const path = call.display.path;
        let state = byPath.get(path);
        if (!state) {
          state = {
            added: 0,
            removed: 0,
            callId: call.id,
            firstCallId: call.id,
            deleted: false,
          };
          byPath.set(path, state);
        }
        if (call.name === "delete") {
          state.removed += call.display.linesRemoved ?? 0;
          state.deleted = true;
        } else {
          if (state.deleted) {
            state.added = 0;
            state.removed = 0;
            state.deleted = false;
            state.firstCallId = call.id;
          }
          state.added += call.display.added ?? 0;
          state.removed += call.display.removed ?? 0;
        }
        state.callId = call.id;
      }
    }
  }
  return [...byPath.entries()]
    .filter(([, state]) => state.deleted || state.added > 0 || state.removed > 0)
    .map(([path, state]) => ({
      path,
      added: state.added,
      removed: state.removed,
      callId: state.callId,
      firstCallId: state.firstCallId,
      deleted: state.deleted,
    }));
};
