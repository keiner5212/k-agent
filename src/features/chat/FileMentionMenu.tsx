import { File, Folder } from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
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

export const FileMentionMenu = ({
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

  useEffect(() => {
    if (!open) return;
    const item = listRef.current?.querySelector<HTMLElement>('[data-active="true"]');
    item?.scrollIntoView({ block: "nearest" });
  }, [open, activeIndex, items]);

  if (!open) return null;

  return (
    <div className="file-mention-menu" role="listbox" aria-label={t("chat.fileMentions.label")}>
      {loading ? (
        <p className="file-mention-menu__empty">{t("chat.fileMentions.loading")}</p>
      ) : error ? (
        <p className="file-mention-menu__empty">{error}</p>
      ) : tooMany ? (
        <p className="file-mention-menu__empty">{t("chat.fileMentions.tooMany")}</p>
      ) : items.length === 0 ? (
        <p className="file-mention-menu__empty">{t("chat.fileMentions.empty")}</p>
      ) : (
        <ul className="file-mention-menu__list" ref={listRef}>
          {items.map((entry, index) => {
            const Icon = entry.kind === "dir" ? Folder : File;
            return (
              <li key={`${entry.kind}:${entry.path}`}>
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
                    {highlightMatch(entry.path, query)}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
};
