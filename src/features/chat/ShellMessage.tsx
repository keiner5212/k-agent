import { useEffect, useMemo, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { parseShellMessage } from "@/lib/shell";
import type { ChatMessage } from "@/types/chat";

export const ShellMessage = ({
  message,
  followTail = true,
}: {
  message: ChatMessage;
  followTail?: boolean;
}): ReactNode => {
  const { t } = useTranslation();
  const { command, output } = useMemo(() => parseShellMessage(message.content), [message.content]);
  const running = message.streaming === true;
  const hasOutput = output.length > 0;
  const outputRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    if (!running || !followTail) return;
    const node = outputRef.current;
    if (!node) return;
    node.scrollTop = node.scrollHeight;
  }, [output, running, followTail]);

  return (
    <div className="chat-message__shell">
      <div className="chat-message__shell-command">
        <span className="chat-message__shell-prompt" aria-hidden="true">
          $
        </span>
        <code className="chat-message__shell-command-text">{command}</code>
      </div>
      <div className="chat-message__shell-body">
        {running ? (
          <span className="visually-hidden" role="status" aria-live="polite">
            {t("chat.shell.running")}
          </span>
        ) : null}
        {running && !hasOutput ? (
          <span className="chat-message__shell-dots" aria-hidden="true">
            <span className="chat-message__shell-dot" />
            <span className="chat-message__shell-dot" />
            <span className="chat-message__shell-dot" />
          </span>
        ) : null}
        {hasOutput ? (
          <pre
            ref={outputRef}
            className={`chat-message__shell-output${
              running ? " chat-message__shell-output--streaming" : ""
            }`}
            aria-busy={running}
          >
            {output}
          </pre>
        ) : null}
      </div>
    </div>
  );
};
