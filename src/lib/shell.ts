import { invoke } from "@tauri-apps/api/core";
import type { Channel } from "@tauri-apps/api/core";
import i18n from "@/i18n";
import { expandWorkspaceMentions } from "@/lib/file-mentions";
import { DESKTOP_REQUIRED, isTauri, platform } from "@/lib/platform";
import type { WorkspaceEntry } from "@/types/workspace-files";

export type ShellChunkKind = "stdout" | "stderr";

export type ShellChunk = {
  kind: ShellChunkKind;
  text: string;
};

export type RunShellRequest = {
  command: string;
  cwd?: string;
  sessionId?: string;
  timeoutMs?: number;
  maxOutputBytes?: number;
};

export type RunShellResponse = {
  command: string;
  cwd: string;
  stdout: string;
  stderr: string;
  exitCode: number | null;
  timedOut: boolean;
  cancelled?: boolean;
  stdoutTruncated: boolean;
  stderrTruncated: boolean;
  durationMs: number;
  startedAtMs: number;
};

export const AI_OUTPUT_LINE_LIMIT = 20;

const POSIX_SAFE = /^[A-Za-z0-9_\-./:=]+$/;
const WIN_SAFE = /^[A-Za-z0-9_\\/.:-]+$/;

export const shellQuote = (value: string): string => {
  if (platform() === "windows") {
    if (value.length === 0) return '""';
    if (WIN_SAFE.test(value)) return value;
    return `"${value.replace(/"/g, '\\"')}"`;
  }
  if (value.length === 0) return "''";
  if (POSIX_SAFE.test(value)) return value;
  let out = "'";
  for (const ch of value) {
    if (ch === "'") out += "'\\''";
    else out += ch;
  }
  out += "'";
  return out;
};

export const expandShellMentions = (
  text: string,
  entries: readonly WorkspaceEntry[],
  workspaceRoot: string,
): string => expandWorkspaceMentions(text, entries, workspaceRoot, shellQuote);

export const runShellCommand = (
  request: RunShellRequest,
  onChunk: Channel<ShellChunk>,
): Promise<RunShellResponse> => {
  if (!isTauri()) return Promise.reject(new Error(DESKTOP_REQUIRED));
  return invoke<RunShellResponse>("run_shell_command", {
    input: {
      command: request.command,
      cwd: request.cwd ?? null,
      sessionId: request.sessionId ?? null,
      timeoutMs: request.timeoutMs ?? null,
      maxOutputBytes: request.maxOutputBytes ?? null,
    },
    onChunk,
  });
};

const buildShellDisplay = (result: RunShellResponse): string => {
  const blocks: string[] = [];
  if (result.stdout.length > 0) blocks.push(result.stdout.replace(/\n$/, ""));
  if (result.stderr.trim().length > 0) {
    blocks.push(`stderr:\n${result.stderr.replace(/\n$/, "")}`);
  }
  const meta: string[] = [];
  if (result.timedOut) meta.push("timed out");
  meta.push(`exit ${result.exitCode ?? "killed"}`);
  meta.push(`${result.durationMs}ms`);
  if (result.stdoutTruncated) meta.push("stdout truncated");
  if (result.stderrTruncated) meta.push("stderr truncated");
  blocks.push(meta.join(" | "));
  return blocks.join("\n\n");
};

export const summarizeShellResultForAi = (result: RunShellResponse): string => {
  const stdoutLines = result.stdout.length === 0 ? 0 : result.stdout.split(/\r?\n/).length;
  const stderrLines = result.stderr.length === 0 ? 0 : result.stderr.split(/\r?\n/).length;
  const totalLines = stdoutLines + stderrLines;
  if (totalLines <= AI_OUTPUT_LINE_LIMIT) {
    return buildShellDisplay(result);
  }
  const flags: string[] = [];
  if (result.timedOut) flags.push("timed out");
  flags.push(`exit ${result.exitCode ?? "killed"}`);
  if (result.stdoutTruncated) flags.push("stdout truncated");
  if (result.stderrTruncated) flags.push("stderr truncated");
  flags.push(`${totalLines} lines`);
  return `Shell command output was too long to show. Run a more targeted command or save the result to a file. (${flags.join(", ")})`;
};

export const buildShellResultContent = (result: RunShellResponse): string =>
  buildShellDisplay(result);

export const formatShellMessage = (command: string, output: string): string =>
  output.length === 0 ? command : `${command}\n\n${output}`;

export const parseShellMessage = (content: string): { command: string; output: string } => {
  const idx = content.indexOf("\n\n");
  if (idx < 0) return { command: content, output: "" };
  return { command: content.slice(0, idx), output: content.slice(idx + 2) };
};

export const appendInterruptedFooter = (output: string): string => {
  const note = i18n.t("chat.shell.interrupted");
  const trimmed = output.replace(/\n+$/, "");
  return trimmed.length > 0 ? `${trimmed}\n\n${note}` : note;
};
