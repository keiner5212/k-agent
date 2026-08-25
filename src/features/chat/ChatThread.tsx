import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles } from "lucide-react";
import { selectActiveMessages, useSessionsStore } from "@/lib/sessions";
import { ChatWaitingLine } from "./ChatWaitingLine";

export const ChatThread = (): ReactNode => {
  const { t } = useTranslation();
  const messages = useSessionsStore(selectActiveMessages);
  const sending = useSessionsStore((state) => state.sending);
  const sendingSessionId = useSessionsStore((state) => state.sendingSessionId);
  const activeSessionId = useSessionsStore((state) => state.activeSessionId);
  const error = useSessionsStore((state) => state.error);
  const waiting = sending && sendingSessionId === activeSessionId;

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
            className={`chat-message chat-message--${message.role}`}
            data-role={message.role}
          >
            <p className="chat-message__content">{message.content}</p>
          </article>
        ))}
        {waiting ? <ChatWaitingLine /> : null}
        {error ? <p className="chat-thread__error">{error}</p> : null}
      </div>
    </section>
  );
};
