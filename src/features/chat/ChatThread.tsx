import { useMemo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles } from "lucide-react";
import { renderMarkdown } from "@/lib/markdown";
import { selectActiveMessages, useSessionsStore } from "@/lib/sessions";
import type { ChatMessage } from "@/types/chat";
import { ChatWaitingLine } from "./ChatWaitingLine";
import { MessageActions } from "./MessageActions";

const AssistantMarkdown = ({ content }: { content: string }): ReactNode => {
  const html = useMemo(() => renderMarkdown(content), [content]);
  if (content.length === 0) return null;
  return (
    <div
      className="chat-message__content chat-message__markdown"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
};

const ThinkingBlock = ({
  reasoning,
  streaming,
  thinkingMs,
}: {
  reasoning: string;
  streaming?: boolean;
  thinkingMs?: number;
}): ReactNode => {
  const { t } = useTranslation();
  if (reasoning.length === 0) return null;
  const seconds =
    !streaming && thinkingMs !== undefined ? Math.max(1, Math.floor(thinkingMs / 1000)) : undefined;
  const label =
    seconds === undefined
      ? t("chat.thinking.label")
      : t("chat.thinking.duration", { count: seconds });
  return (
    <details className="chat-thinking" open={Boolean(streaming)}>
      <summary className="chat-thinking__summary">{label}</summary>
      <pre className="chat-thinking__body">{reasoning}</pre>
    </details>
  );
};

const MessageBody = ({ message }: { message: ChatMessage }): ReactNode => {
  if (message.kind === "shell") {
    return <pre className="chat-message__content chat-message__shell">{message.content}</pre>;
  }
  if (message.role === "assistant") {
    return (
      <>
        <ThinkingBlock
          reasoning={message.reasoning ?? ""}
          streaming={message.streaming}
          thinkingMs={message.thinkingMs}
        />
        <AssistantMarkdown content={message.content} />
      </>
    );
  }
  return <p className="chat-message__content">{message.content}</p>;
};

export const ChatThread = (): ReactNode => {
  const { t } = useTranslation();
  const messages = useSessionsStore(selectActiveMessages);
  const sending = useSessionsStore((state) => state.sending);
  const sendingSessionId = useSessionsStore((state) => state.sendingSessionId);
  const activeSessionId = useSessionsStore((state) => state.activeSessionId);
  const error = useSessionsStore((state) => state.error);
  const waiting =
    sending &&
    sendingSessionId === activeSessionId &&
    !messages.some((message) => message.streaming);

  if (messages.length === 0 && !error) {
    return (
      <section className="chat-thread" aria-live="polite">
        <div className="chat-thread__empty">
          <Sparkles size={20} strokeWidth={1.5} />
          <h2 className="chat-thread__empty-title">{t("chat.thread.emptyTitle")}</h2>
          <p className="chat-thread__empty-description">{t("chat.thread.emptyDescription")}</p>
        </div>
      </section>
    );
  }

  return (
    <section className="chat-thread chat-thread--active" aria-live="polite">
      <div className="chat-thread__messages">
        {messages.map((message) => (
          <article
            key={message.id}
            className={`chat-message chat-message--${message.role}${message.kind === "shell" ? " chat-message--shell" : ""}${message.streaming ? " chat-message--streaming" : ""}`}
            data-role={message.role}
          >
            <MessageBody message={message} />
            <MessageActions message={message} />
          </article>
        ))}
        {waiting ? <ChatWaitingLine /> : null}
        {error ? <p className="chat-thread__error">{error}</p> : null}
      </div>
    </section>
  );
};
