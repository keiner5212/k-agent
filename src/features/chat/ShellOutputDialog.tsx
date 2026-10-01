import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Copy, Terminal } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { IconButton } from "@/components/IconButton";
import { parseShellMessage } from "@/lib/shell";
import { useShellOutputStore } from "@/lib/shell-output";
import { useSessionsStore } from "@/lib/sessions";

const COPY_FEEDBACK_MS = 1200;

const copyText = async (text: string): Promise<boolean> => {
  if (typeof navigator === "undefined" || !navigator.clipboard) return false;
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

const STDERR_MARK = "stderr:\n";

const splitOutputBlocks = (output: string): Array<{ kind: "stdout" | "stderr"; text: string }> => {
  if (output.length === 0) return [];
  if (output.startsWith(STDERR_MARK)) {
    return [{ kind: "stderr", text: output.slice(STDERR_MARK.length) }];
  }
  const sep = `\n\n${STDERR_MARK}`;
  const idx = output.indexOf(sep);
  if (idx < 0) return [{ kind: "stdout", text: output }];
  const blocks: Array<{ kind: "stdout" | "stderr"; text: string }> = [];
  const stdout = output.slice(0, idx);
  const stderr = output.slice(idx + sep.length);
  if (stdout.length > 0) blocks.push({ kind: "stdout", text: stdout });
  if (stderr.length > 0) blocks.push({ kind: "stderr", text: stderr });
  return blocks;
};

export const ShellOutputDialog = (): ReactNode => {
  const { t } = useTranslation();
  const open = useShellOutputStore((state) => state.open);
  const messageId = useShellOutputStore((state) => state.messageId);
  const sessionId = useShellOutputStore((state) => state.sessionId);
  const command = useShellOutputStore((state) => state.command);
  const close = useShellOutputStore((state) => state.close);
  const message = useSessionsStore((state) => {
    if (!open || !sessionId || !messageId) return null;
    const session = state.sessions.find((item) => item.id === sessionId);
    return session?.messages.find((item) => item.id === messageId) ?? null;
  });
  const content = message?.content ?? "";
  const isStreaming = Boolean(message?.streaming);
  const missing = open && !message;
  const parsed = useMemo(() => parseShellMessage(content), [content]);
  const blocks = useMemo(() => splitOutputBlocks(parsed.output), [parsed.output]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const copyTimerRef = useRef(0);
  const [copied, setCopied] = useState(false);

  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (!next) close();
    },
    [close],
  );

  useEffect(() => {
    if (missing) close();
  }, [missing, close]);

  useEffect(() => {
    return () => window.clearTimeout(copyTimerRef.current);
  }, []);

  useEffect(() => {
    if (!isStreaming) return;
    const node = scrollRef.current;
    if (!node) return;
    node.scrollTop = node.scrollHeight;
  }, [blocks, isStreaming]);

  const handleCopy = useCallback(async () => {
    const text = parsed.output.length > 0 ? parsed.output : content;
    const ok = await copyText(text);
    if (!ok) return;
    setCopied(true);
    window.clearTimeout(copyTimerRef.current);
    copyTimerRef.current = window.setTimeout(() => setCopied(false), COPY_FEEDBACK_MS);
  }, [content, parsed.output]);

  return (
    <Dialog
      open={open}
      onOpenChange={handleOpenChange}
      titleKey="chat.shell.console.title"
      size="wide"
    >
      <div className="shell-console">
        <header className="shell-console__head">
          <Terminal size={14} strokeWidth={1.5} className="shell-console__icon" />
          <span className="shell-console__prompt" aria-hidden="true">
            $
          </span>
          <code className="shell-console__command">{command}</code>
          <IconButton
            label={copied ? t("chat.shell.console.copied") : t("chat.shell.console.copy")}
            className="shell-console__copy"
            onClick={() => {
              void handleCopy();
            }}
          >
            <Copy size={14} strokeWidth={1.5} />
          </IconButton>
        </header>
        <div
          ref={scrollRef}
          className={`shell-console__output${isStreaming ? " shell-console__output--streaming" : ""}`}
        >
          {blocks.length === 0 ? (
            <p className="shell-console__placeholder">{t("chat.shell.console.empty")}</p>
          ) : (
            blocks.map((block) => (
              <pre
                key={block.kind}
                className={`shell-console__block shell-console__block--${block.kind}`}
              >
                {block.text}
              </pre>
            ))
          )}
        </div>
      </div>
    </Dialog>
  );
};
