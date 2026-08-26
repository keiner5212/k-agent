import { invoke } from "@tauri-apps/api/core";
import type { ChatAttachment, ChatMessage, ToolDisplay } from "@/types/chat";

export type SessionChangedFile = {
  path: string;
  added: number;
  removed: number;
  callId: string;
};

export const yamlBlockValue = (source: string, key: string): string => {
  const markers = [`${key}: |-\n`, `${key}: |\n`];
  let rest = "";
  for (const marker of markers) {
    const idx = source.indexOf(marker);
    if (idx < 0) continue;
    rest = source.slice(idx + marker.length);
    break;
  }
  if (!rest) return "";
  const lines = rest.split("\n");
  const body: string[] = [];
  for (const line of lines) {
    if (line.startsWith("  ")) {
      body.push(line.slice(2));
      continue;
    }
    if (line.trim() === "") {
      body.push("");
      continue;
    }
    break;
  }
  while (body.length > 0 && body[body.length - 1] === "") body.pop();
  return body.join("\n");
};

export const unifiedDiff = (path: string, before: string, after: string, context = 3): string => {
  const oldLines = before.split("\n");
  const newLines = after.split("\n");
  if (before === after) return `--- a/${path}\n+++ b/${path}\n`;
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
  const hunkEndNew = Math.min(newLines.length, newEnd + context);
  const oldCount = hunkEndOld - hunkStartOld;
  const newCount = hunkEndNew - hunkStartNew;
  const lines = [
    `--- a/${path}`,
    `+++ b/${path}`,
    `@@ -${hunkStartOld + 1},${oldCount} +${hunkStartNew + 1},${newCount} @@`,
  ];
  for (let index = hunkStartOld; index < start; index += 1) {
    lines.push(` ${oldLines[index]}`);
  }
  for (let index = start; index < oldEnd; index += 1) {
    lines.push(`-${oldLines[index] ?? ""}`);
  }
  for (let index = start; index < newEnd; index += 1) {
    lines.push(`+${newLines[index] ?? ""}`);
  }
  const tailOld = oldLines.slice(oldEnd, hunkEndOld);
  for (const line of tailOld) {
    lines.push(` ${line}`);
  }
  return lines.join("\n");
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

export const collectSessionChanges = (messages: ChatMessage[]): SessionChangedFile[] => {
  const byPath = new Map<string, SessionChangedFile>();
  for (const message of messages) {
    for (const round of message.toolRounds ?? []) {
      for (const call of round.calls) {
        if (!isActionOk(call.display) || !call.display.path) continue;
        const path = call.display.path;
        const prev = byPath.get(path);
        byPath.set(path, {
          path,
          added: (prev?.added ?? 0) + (call.display.added ?? 0),
          removed: (prev?.removed ?? 0) + (call.display.removed ?? 0),
          callId: call.id,
        });
      }
    }
  }
  return [...byPath.values()];
};
