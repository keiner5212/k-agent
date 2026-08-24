import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles } from "lucide-react";

export const ChatThread = (): ReactNode => {
  const { t } = useTranslation();
  return (
    <section className="chat-thread" aria-live="polite">
      <div className="chat-thread__empty">
        <Sparkles size={20} strokeWidth={1.5} />
        <h2 className="chat-thread__empty-title">{t("chat.thread.emptyTitle")}</h2>
        <p className="chat-thread__empty-description">
          {t("chat.thread.emptyDescription")}
        </p>
      </div>
    </section>
  );
};