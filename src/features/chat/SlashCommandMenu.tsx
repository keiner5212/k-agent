import { memo, useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles, Terminal } from "lucide-react";
import { highlightMatch } from "@/lib/highlight";
import { formatInlineTokenCount } from "@/lib/skill-mentions";
import type { SlashPickItem } from "./use-slash-commands";

type SlashCommandMenuProps = {
  open: boolean;
  items: readonly SlashPickItem[];
  query: string;
  activeIndex: number;
  loading: boolean;
  error?: string;
  tooMany: boolean;
  onPick: (item: SlashPickItem) => void;
};

export const SlashCommandMenu = memo(
  ({
    open,
    items,
    query,
    activeIndex,
    loading,
    error,
    tooMany,
    onPick,
  }: SlashCommandMenuProps): ReactNode => {
    const { t } = useTranslation();
    const listRef = useRef<HTMLUListElement>(null);

    useEffect(() => {
      if (!open) return;
      const item = listRef.current?.querySelector<HTMLElement>('[data-active="true"]');
      if (!item) return;
      const root = listRef.current;
      if (!root) return;
      const itemTop = item.offsetTop;
      const itemBottom = itemTop + item.offsetHeight;
      if (itemTop < root.scrollTop) root.scrollTop = itemTop;
      else if (itemBottom > root.scrollTop + root.clientHeight) {
        root.scrollTop = itemBottom - root.clientHeight;
      }
    }, [open, activeIndex]);

    if (!open) return null;

    const showList = items.length > 0 && !tooMany && !error;

    return (
      <div className="file-mention-menu" role="listbox" aria-label={t("chat.slashCommands.label")}>
        {showList ? (
          <ul className="file-mention-menu__list" ref={listRef}>
            {items.map((item, index) => {
              if (item.kind === "skill") {
                return (
                  <li key={`skill:${item.skill.id}`}>
                    <button
                      type="button"
                      className="file-mention-menu__item"
                      data-active={index === activeIndex ? "true" : "false"}
                      role="option"
                      aria-selected={index === activeIndex}
                      onMouseDown={(event) => {
                        event.preventDefault();
                        onPick(item);
                      }}
                    >
                      <Sparkles size={14} strokeWidth={1.5} className="file-mention-menu__icon" />
                      <span className="file-mention-menu__path">
                        {highlightMatch(item.skill.name, query)}
                        <span className="file-mention-menu__meta">
                          {formatInlineTokenCount(item.skill.estimatedTokens)}
                        </span>
                      </span>
                      <span className="file-mention-menu__hint">{item.skill.description}</span>
                    </button>
                  </li>
                );
              }
              const command = item.command;
              return (
                <li key={command.id}>
                  <button
                    type="button"
                    className="file-mention-menu__item"
                    data-active={index === activeIndex ? "true" : "false"}
                    role="option"
                    aria-selected={index === activeIndex}
                    onMouseDown={(event) => {
                      event.preventDefault();
                      onPick(item);
                    }}
                  >
                    <Terminal size={14} strokeWidth={1.5} className="file-mention-menu__icon" />
                    <span className="file-mention-menu__path">
                      {highlightMatch(command.name, query)}
                      <span className="file-mention-menu__meta">
                        {formatInlineTokenCount(command.estimatedTokens)}
                      </span>
                    </span>
                    <span className="file-mention-menu__hint">{t(command.descriptionKey)}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        ) : loading ? (
          <p className="file-mention-menu__empty">{t("chat.slashCommands.loading")}</p>
        ) : error ? (
          <p className="file-mention-menu__empty">{error}</p>
        ) : tooMany ? (
          <p className="file-mention-menu__empty">{t("chat.slashCommands.tooMany")}</p>
        ) : (
          <p className="file-mention-menu__empty">{t("chat.slashCommands.empty")}</p>
        )}
      </div>
    );
  },
);

SlashCommandMenu.displayName = "SlashCommandMenu";
