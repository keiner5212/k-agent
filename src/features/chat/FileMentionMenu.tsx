import { memo, useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { File, Folder } from "lucide-react";
import { highlightMatch } from "@/lib/highlight";
import type { WorkspaceEntry } from "@/types/workspace-files";

type FileMentionMenuProps = {
  open: boolean;
  items: readonly WorkspaceEntry[];
  query: string;
  activeIndex: number;
  loading: boolean;
  error?: string;
  tooMany: boolean;
  onPick: (entry: WorkspaceEntry) => void;
};

const highlightQuery = (query: string): string => {
  const slash = query.lastIndexOf("/");
  if (slash === -1) return query;
  return query.slice(slash + 1);
};

export const FileMentionMenu = memo(
  ({
    open,
    items,
    query,
    activeIndex,
    loading,
    error,
    tooMany,
    onPick,
  }: FileMentionMenuProps): ReactNode => {
    const { t } = useTranslation();
    const listRef = useRef<HTMLUListElement>(null);
    const prefix = highlightQuery(query);

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
      <div className="file-mention-menu" role="listbox" aria-label={t("chat.fileMentions.label")}>
        {showList ? (
          <ul className="file-mention-menu__list" ref={listRef}>
            {items.map((entry, index) => {
              const Icon = entry.kind === "dir" ? Folder : File;
              return (
                <li key={entry.path}>
                  <button
                    type="button"
                    className="file-mention-menu__item"
                    data-active={index === activeIndex ? "true" : "false"}
                    role="option"
                    aria-selected={index === activeIndex}
                    onMouseDown={(event) => {
                      event.preventDefault();
                      onPick(entry);
                    }}
                  >
                    <Icon size={14} strokeWidth={1.5} className="file-mention-menu__icon" />
                    <span className="file-mention-menu__path">
                      {highlightMatch(entry.path, prefix)}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        ) : loading ? (
          <p className="file-mention-menu__empty">{t("chat.fileMentions.loading")}</p>
        ) : error ? (
          <p className="file-mention-menu__empty">{error}</p>
        ) : tooMany ? (
          <p className="file-mention-menu__empty">{t("chat.fileMentions.tooMany")}</p>
        ) : (
          <p className="file-mention-menu__empty">{t("chat.fileMentions.empty")}</p>
        )}
      </div>
    );
  },
);

FileMentionMenu.displayName = "FileMentionMenu";
