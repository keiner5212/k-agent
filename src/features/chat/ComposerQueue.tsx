import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ChevronsRight, X } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { useSessionsStore } from "@/lib/sessions";

export const ComposerQueue = (): ReactNode => {
  const { t } = useTranslation();
  const activeSessionId = useSessionsStore((state) => state.activeSessionId);
  const queued = useSessionsStore((state) => state.queued);
  const removeQueued = useSessionsStore((state) => state.removeQueued);
  const sendQueuedNow = useSessionsStore((state) => state.sendQueuedNow);
  const items = queued.filter((item) => item.sessionId === activeSessionId);
  if (items.length === 0) return null;

  return (
    <div className="composer-queue" aria-live="polite">
      <ul className="composer-queue__list">
        {items.map((item) => (
          <li key={item.id} className="composer-queue__item">
            {item.mode === "shell" ? (
              <span className="composer-queue__mode" aria-hidden="true">
                $
              </span>
            ) : null}
            <span className="composer-queue__text" title={item.text}>
              {item.text}
            </span>
            <GlassButton
              variant="secondary"
              className="composer-queue__now"
              onClick={() => {
                void sendQueuedNow(item.id);
              }}
            >
              <ChevronsRight size={14} strokeWidth={2} />
              {t("chat.queue.sendNow")}
            </GlassButton>
            <IconButton
              label={t("chat.queue.remove")}
              className="composer-queue__remove"
              onClick={() => removeQueued(item.id)}
            >
              <X size={14} strokeWidth={1.5} />
            </IconButton>
          </li>
        ))}
      </ul>
    </div>
  );
};
