import { invoke } from "@tauri-apps/api/core";
import type { ChatAttachment, ChatMessage, ToolDisplay } from "@/types/chat";

export type SessionChangedFile = {
  path: string;
  added: number;
  removed: number;
  callId: string;
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

export type DiffLineKind = "context" | "add" | "remove";

type DiffRow = {
  kind: DiffLineKind;
  text: string;
  line: number;
};

export type DiffEditorValue = {
  value: string;
  lineNumbers: number[];
  lineKinds: DiffLineKind[];
};

const splitLines = (source: string): string[] => (source.length === 0 ? [] : source.split("\n"));

const buildDiffRows = (before: string, after: string, context: number): DiffRow[] => {
  const oldLines = splitLines(before);
  const newLines = splitLines(after);
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
  const hunkStartOld = Math.max(0, start - context);
  const hunkStartNew = Math.max(0, start - context);
  const hunkEndOld = Math.min(oldLines.length, oldEnd + context);
  const rows: DiffRow[] = [];
  let oldLine = hunkStartOld + 1;
  let newLine = hunkStartNew + 1;
  for (let index = hunkStartOld; index < start; index += 1) {
    rows.push({ kind: "context", text: oldLines[index] ?? "", line: newLine });
    oldLine += 1;
    newLine += 1;
  }
  for (let index = start; index < oldEnd; index += 1) {
    rows.push({ kind: "remove", text: oldLines[index] ?? "", line: oldLine });
    oldLine += 1;
  }
  for (let index = start; index < newEnd; index += 1) {
    rows.push({ kind: "add", text: newLines[index] ?? "", line: newLine });
    newLine += 1;
  }
  for (let index = oldEnd; index < hunkEndOld; index += 1) {
    rows.push({ kind: "context", text: oldLines[index] ?? "", line: newLine });
    newLine += 1;
  }
  return rows;
};

export const diffEditorValue = (before: string, after: string, context = 3): DiffEditorValue => {
  if (before === after) {
    return { value: after, lineNumbers: [], lineKinds: [] };
  }
  const rows = buildDiffRows(before, after, context);
  return {
    value: rows.map((row) => row.text).join("\n"),
    lineNumbers: rows.map((row) => row.line),
    lineKinds: rows.map((row) => row.kind),
  };
};

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
  deleted: boolean;
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
          state = { added: 0, removed: 0, callId: call.id, deleted: false };
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
          }
          state.added += call.display.added ?? 0;
          state.removed += call.display.removed ?? 0;
        }
        state.callId = call.id;
      }
    }
  }
  return [...byPath.entries()]
    .filter(([, state]) => state.added !== state.removed)
    .map(([path, { added, removed, callId }]) => ({ path, added, removed, callId }));
};
