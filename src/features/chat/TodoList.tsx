import { Check, Circle, CircleDashed, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { TodoItem as TodoItemType } from "@/types/chat";

type TodoListProps = {
  todos: TodoItemType[];
};

const STATUS_ICON = {
  pending: CircleDashed,
  in_progress: Circle,
  completed: Check,
  cancelled: X,
} as const;

const priorityBand = (priority: number): "high" | "medium" | "low" => {
  if (priority >= 8) return "high";
  if (priority >= 4) return "medium";
  return "low";
};

const TodoList = ({ todos }: TodoListProps): React.ReactNode => {
  const { t } = useTranslation();
  if (todos.length === 0) {
    return null;
  }
  const pendingCount = todos.filter(
    (item) => item.status !== "completed" && item.status !== "cancelled",
  ).length;
  return (
    <section className="chat-todos" aria-label={t("chat.todos.title")}>
      <header className="chat-todos__head">
        <span className="chat-todos__title">{t("chat.todos.title")}</span>
        <span className="chat-todos__count">
          {pendingCount}/{todos.length}
        </span>
      </header>
      <ul className="chat-todos__list">
        {todos.map((item) => {
          const Icon = STATUS_ICON[item.status] ?? CircleDashed;
          const statusLabel = t(`chat.todos.status.${item.status}`);
          const band = priorityBand(item.priority);
          const priorityLabel = t(`chat.todos.priority.${band}`);
          const safePriority = Math.max(0, Math.min(10, Math.round(item.priority)));
          return (
            <li
              key={item.id}
              className={`chat-todos__item chat-todos__item--${item.status}`}
              data-priority-band={band}
            >
              <span className="chat-todos__icon" aria-hidden="true">
                <Icon size={14} strokeWidth={1.75} />
              </span>
              <span className="chat-todos__content">{item.content}</span>
              <span className="chat-todos__meta">
                <span className="chat-todos__status">{statusLabel}</span>
                <span
                  className="chat-todos__priority"
                  title={`${priorityLabel} (${safePriority}/10)`}
                >
                  {priorityLabel} {safePriority}/10
                </span>
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
};

export { TodoList };
export type { TodoListProps };
