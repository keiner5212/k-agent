import { useMemo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquarePlus, Trash2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
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
              title={session.title || t("sessions.untitled")}
              preview={session.preview}
              active={session.id === activeSessionId}
              onSelect={() => select(session.id)}
              onDelete={() => remove(session.id)}
            />
          ))}
        </ul>
      )}
    </aside>
  );
};

type SessionItemProps = {
  title: string;
  preview: string;
  active: boolean;
  onSelect: () => void;
  onDelete: () => void;
};

const SessionItem = ({
  title,
  preview,
  active,
  onSelect,
  onDelete,
}: SessionItemProps): ReactNode => {
  const { t } = useTranslation();
  const previewText =
    preview.trim().length > 0 ? titleFromFirstMessage(preview) : t("sessions.noPreview");

  return (
    <li className="sessions-sidebar__item-wrap">
      <button
        type="button"
        className="sessions-sidebar__item"
        data-active={active ? "true" : "false"}
        onClick={onSelect}
      >
        <span className="sessions-sidebar__item-title">{title}</span>
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
        <Trash2 size={12} strokeWidth={1.5} />
      </IconButton>
    </li>
  );
};
