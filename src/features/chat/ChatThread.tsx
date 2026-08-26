import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FileText, Film, Sparkles } from "lucide-react";
import { attachmentPreviewUrl } from "@/lib/attachments";
import { renderMarkdown } from "@/lib/markdown";
import { INTERRUPT_ARM_MS, selectActiveMessages, useSessionsStore } from "@/lib/sessions";
import type { ChatAttachment, ChatMessage, ChatToolCall } from "@/types/chat";
import { AttachmentPreviewDialog } from "./AttachmentPreviewDialog";
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

const formatToolCall = (call: ChatToolCall): string =>
  call.argument && call.argument.length > 0 ? `${call.name} "${call.argument}"` : call.name;

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

const ToolCallsBlock = ({ calls }: { calls: ChatToolCall[] }): ReactNode => {
  if (calls.length === 0) return null;
  return (
    <ul className="chat-tools">
      {calls.map((call, index) => (
        <li key={call.id ?? `${call.name}-${index}`} className="chat-tools__item">
          {formatToolCall(call)}
        </li>
      ))}
    </ul>
  );
};

const InterruptedFooter = ({ interrupted }: { interrupted: boolean }): ReactNode => {
  const { t } = useTranslation();
  if (!interrupted) return null;
  return <p className="chat-message__interrupted">{t("chat.interruptedByUser")}</p>;
};

const MessageAttachments = ({ items }: { items: ChatAttachment[] }): ReactNode => {
  const [preview, setPreview] = useState<ChatAttachment | null>(null);
  if (items.length === 0) return null;
  return (
    <>
      <ul className="chat-message__attachments">
        {items.map((item) => {
          const thumb = attachmentPreviewUrl(item);
          return (
            <li key={item.id} className="chat-message__attachment">
              <button
                type="button"
                className="chat-message__attachment-open"
                title={item.name}
                onClick={() => setPreview(item)}
              >
                {thumb ? (
                  <img src={thumb} alt={item.name} className="chat-message__attachment-image" />
                ) : (
                  <span className="chat-message__attachment-file">
                    {item.kind === "video" ? (
                      <Film size={14} strokeWidth={1.5} />
                    ) : (
                      <FileText size={14} strokeWidth={1.5} />
                    )}
                    <span>{item.name}</span>
                  </span>
                )}
              </button>
            </li>
          );
        })}
      </ul>
      <AttachmentPreviewDialog item={preview} onClose={() => setPreview(null)} />
    </>
  );
};

const MessageBody = ({ message }: { message: ChatMessage }): ReactNode => {
  if (message.kind === "shell") {
    return (
      <>
        <pre className="chat-message__content chat-message__shell">{message.content}</pre>
        <InterruptedFooter interrupted={message.interrupted ?? false} />
      </>
    );
  }
  if (message.role === "assistant") {
    return (
      <>
        <ThinkingBlock
          reasoning={message.reasoning ?? ""}
          streaming={message.streaming}
          thinkingMs={message.thinkingMs}
        />
        <ToolCallsBlock calls={message.toolCalls ?? []} />
        <AssistantMarkdown content={message.content} />
        <InterruptedFooter interrupted={message.interrupted ?? false} />
      </>
    );
  }
  return (
    <>
      <MessageAttachments items={message.attachments ?? []} />
      {message.content ? <p className="chat-message__content">{message.content}</p> : null}
    </>
  );
};

const InterruptHint = (): ReactNode => {
  const { t } = useTranslation();
  const armed = useSessionsStore((state) => state.interruptArmedAt);
  const sending = useSessionsStore((state) => state.sending);
  const shellRunning = useSessionsStore((state) => state.shellRunning);

  useEffect(() => {
    if (armed === null) return;
    const remain = INTERRUPT_ARM_MS - (Date.now() - armed);
    if (remain <= 0) {
      useSessionsStore.getState().disarmInterrupt();
      return;
    }
    const timer = window.setTimeout(() => {
      useSessionsStore.getState().disarmInterrupt();
    }, remain);
    return () => window.clearTimeout(timer);
  }, [armed]);

  if (armed === null || (!sending && !shellRunning)) return null;
  return (
    <div className="chat-interrupt-hint" role="status" aria-live="polite">
      <span className="chat-interrupt-hint__dot" aria-hidden="true" />
      <span>{t("chat.interruptHint")}</span>
    </div>
  );
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
            className={`chat-message chat-message--${message.role}${message.kind === "shell" ? " chat-message--shell" : ""}${message.streaming ? " chat-message--streaming" : ""}${message.interrupted ? " chat-message--interrupted" : ""}`}
            data-role={message.role}
          >
            <MessageBody message={message} />
            <MessageActions message={message} />
          </article>
        ))}
        {waiting ? <ChatWaitingLine /> : null}
        <InterruptHint />
        {error ? <p className="chat-thread__error">{error}</p> : null}
      </div>
    </section>
  );
};
