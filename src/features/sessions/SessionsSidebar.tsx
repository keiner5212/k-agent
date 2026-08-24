import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquarePlus, Trash2 } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { type SessionSummary } from "@/types/sessions";
import { useSessionsStore } from "@/lib/sessions";

export const SessionsSidebar = (): ReactNode => {
  const { t } = useTranslation();
  const sessions = useSessionsStore((state) => state.sessions);
  const remove = useSessionsStore((state) => state.remove);

  return (
    <aside className="sessions-sidebar" aria-label={t("sessions.title")}>
      <div className="sessions-sidebar__head">
        <span className="sessions-sidebar__title">{t("sessions.title")}</span>
      </div>
      <button type="button" className="sessions-sidebar__new" aria-label={t("sessions.new")}>
        <MessageSquarePlus size={14} strokeWidth={1.5} />
        <span>{t("sessions.new")}</span>
      </button>
      {sessions.length === 0 ? (
        <p className="sessions-sidebar__empty">{t("sessions.empty")}</p>
      ) : (
        <ul className="sessions-sidebar__list">
          {sessions.map((session, index) => (
            <SessionItem
              key={session.id}
              session={session}
              active={index === 0}
              onDelete={() => remove(session.id)}
            />
          ))}
        </ul>
      )}
    </aside>
  );
};

type SessionItemProps = {
  session: SessionSummary;
  active: boolean;
  onDelete: () => void;
};

const SessionItem = ({ session, active, onDelete }: SessionItemProps): ReactNode => {
  const { t } = useTranslation();
  return (
    <li className="sessions-sidebar__item-wrap">
      <button
        type="button"
        className="sessions-sidebar__item"
        data-active={active ? "true" : "false"}
      >
        <span className="sessions-sidebar__item-title">{session.title}</span>
      </button>
      <IconButton
        label={t("sessions.delete")}
        className="sessions-sidebar__item-delete"
        onClick={onDelete}
      >
        <Trash2 size={12} strokeWidth={1.5} />
      </IconButton>
    </li>
  );
};
