import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquarePlus, Trash2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { APP_VERSION } from "@/lib/app-meta";
import { useSessionsStore } from "@/lib/sessions";
import { sortSessions, titleFromFirstMessage } from "@/types/sessions";

export const SessionsSidebar = (): ReactNode => {
  const { t } = useTranslation();
  const sessions = useSessionsStore((state) => state.sessions);
  const sortedSessions = useMemo(() => sortSessions(sessions), [sessions]);
  const activeSessionId = useSessionsStore((state) => state.activeSessionId);
  const create = useSessionsStore((state) => state.create);
  const select = useSessionsStore((state) => state.select);
  const remove = useSessionsStore((state) => state.remove);

  return (
    <aside className="sessions-sidebar" aria-label={t("sessions.title")}>
      <div className="sessions-sidebar__head">
        <span className="sessions-sidebar__title">{t("sessions.title")}</span>
      </div>
      <GlassButton
        variant="primary"
        className="sessions-sidebar__new"
        aria-label={t("sessions.new")}
        onClick={() => create()}
      >
        <MessageSquarePlus strokeWidth={1.5} />
        {t("sessions.new")}
      </GlassButton>
      {sortedSessions.length === 0 ? (
        <p className="sessions-sidebar__empty">{t("sessions.empty")}</p>
      ) : (
        <ul className="sessions-sidebar__list">
          {sortedSessions.map((session) => (
            <SessionItem
              key={session.id}
              title={session.title}
              untitled={t("sessions.untitled")}
              preview={session.preview}
              active={session.id === activeSessionId}
              onSelect={() => select(session.id)}
              onDelete={() => remove(session.id)}
            />
          ))}
        </ul>
      )}
      <p className="sessions-sidebar__foot">{t("about.version", { version: APP_VERSION })}</p>
    </aside>
  );
};

type SessionItemProps = {
  title: string;
  untitled: string;
  preview: string;
  active: boolean;
  onSelect: () => void;
  onDelete: () => void;
};

const SessionItem = ({
  title,
  untitled,
  preview,
  active,
  onSelect,
  onDelete,
}: SessionItemProps): ReactNode => {
  const { t } = useTranslation();
  const [reveal, setReveal] = useState(false);
  const prevTitleRef = useRef(title);
  const previewText =
    preview.trim().length > 0 ? titleFromFirstMessage(preview) : t("sessions.noPreview");
  const display = title.trim().length > 0 ? title : untitled;

  useLayoutEffect(() => {
    const wasEmpty = prevTitleRef.current.trim().length === 0;
    prevTitleRef.current = title;
    if (wasEmpty && title.trim().length > 0) {
      setReveal(true);
    }
  }, [title]);

  return (
    <li className="sessions-sidebar__item-wrap" data-active={active ? "true" : "false"}>
      <button type="button" className="sessions-sidebar__item" onClick={onSelect}>
        <span
          className={
            reveal
              ? "sessions-sidebar__item-title sessions-sidebar__item-title--reveal"
              : "sessions-sidebar__item-title"
          }
          onAnimationEnd={() => setReveal(false)}
        >
          {display}
        </span>
        <span className="sessions-sidebar__item-preview">{previewText}</span>
      </button>
      <IconButton
        label={t("sessions.delete")}
        className="sessions-sidebar__item-delete"
        onMouseDown={(event) => {
          event.preventDefault();
          event.stopPropagation();
        }}
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onDelete();
        }}
      >
        <Trash2 size={14} strokeWidth={1.75} />
      </IconButton>
    </li>
  );
};
