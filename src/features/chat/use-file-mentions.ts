import { useCallback, useEffect, useMemo, useState, useDeferredValue } from "react";
import {
  applyMentionSelection,
  buildMentionPathSet,
  filterWorkspaceEntries,
  parseActiveMention,
  type ActiveMention,
} from "@/lib/file-mentions";
import { useWorkspaceFilesStore, workspaceFilesLoading } from "@/lib/workspace-files";
import type { WorkspaceEntry } from "@/types/workspace-files";

type UseFileMentionsArgs = {
  value: string;
  cursor: number;
  onApply: (next: string, cursor: number) => void;
};

type UseFileMentionsResult = {
  menuOpen: boolean;
  items: WorkspaceEntry[];
  tooMany: boolean;
  activeIndex: number;
  activeMention: ActiveMention | null;
  mentionPaths: Set<string>;
  loading: boolean;
  error?: string;
  handleKeyDown: (event: React.KeyboardEvent<HTMLTextAreaElement>) => boolean;
  pickItem: (entry: WorkspaceEntry) => void;
  resetMenu: () => void;
};

type MentionSelection = {
  key: string;
  index: number;
};

export const useFileMentions = ({
  value,
  cursor,
  onApply,
}: UseFileMentionsArgs): UseFileMentionsResult => {
  const entries = useWorkspaceFilesStore((state) => state.entries);
  const deferredEntries = useDeferredValue(entries);
  const loading = useWorkspaceFilesStore(workspaceFilesLoading);
  const error = useWorkspaceFilesStore((state) => state.error);
  const ensureRootLoaded = useWorkspaceFilesStore((state) => state.ensureRootLoaded);
  const ensureMentionScope = useWorkspaceFilesStore((state) => state.ensureMentionScope);
  const [selection, setSelection] = useState<MentionSelection>({ key: "", index: 0 });
  const [dismissedKey, setDismissedKey] = useState<string | null>(null);

  const activeMention = useMemo(() => parseActiveMention(value, cursor), [value, cursor]);
  const mentionKey = activeMention ? `${activeMention.start}:${activeMention.query}` : "";
  const menuOpen = mentionKey.length > 0 && dismissedKey !== mentionKey;
  const activeIndex = selection.key === mentionKey ? selection.index : 0;

  const filterResult = useMemo(() => {
    if (!activeMention) return { items: [], tooMany: false };
    return filterWorkspaceEntries(deferredEntries, activeMention.query);
  }, [activeMention, deferredEntries]);
  const items = filterResult.items;
  const tooMany = filterResult.tooMany;

  const mentionPaths = useMemo(() => buildMentionPathSet(entries), [entries]);

  useEffect(() => {
    void ensureRootLoaded();
  }, [ensureRootLoaded]);

  useEffect(() => {
    if (!activeMention) return;
    void ensureMentionScope(activeMention.query);
  }, [activeMention, ensureMentionScope]);

  const setActiveIndex = useCallback(
    (next: number | ((prev: number) => number)) => {
      setSelection((prev) => {
        const base = prev.key === mentionKey ? prev.index : 0;
        const index = typeof next === "function" ? next(base) : next;
        return { key: mentionKey, index };
      });
    },
    [mentionKey],
  );

  const pickItem = useCallback(
    (entry: WorkspaceEntry) => {
      if (!activeMention) return;
      const { next, cursor: nextCursor } = applyMentionSelection(value, activeMention, entry);
      onApply(next, nextCursor);
      setSelection({ key: "", index: 0 });
      setDismissedKey(null);
    },
    [activeMention, onApply, value],
  );

  const resetMenu = useCallback(() => {
    setDismissedKey(mentionKey);
    setSelection({ key: mentionKey, index: 0 });
  }, [mentionKey]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLTextAreaElement>): boolean => {
      if (!menuOpen || tooMany) return false;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        if (items.length === 0) return true;
        setActiveIndex((prev) => (prev + 1) % items.length);
        return true;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        if (items.length === 0) return true;
        setActiveIndex((prev) => (prev - 1 + items.length) % items.length);
        return true;
      }
      if (event.key === "Enter" && !event.shiftKey) {
        if (items.length === 0) return false;
        event.preventDefault();
        const entry = items[activeIndex];
        if (entry) pickItem(entry);
        return true;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        resetMenu();
        return true;
      }
      return false;
    },
    [activeIndex, items, menuOpen, pickItem, resetMenu, setActiveIndex, tooMany],
  );

  return {
    menuOpen,
    items,
    tooMany,
    activeIndex,
    activeMention,
    mentionPaths,
    loading,
    error,
    handleKeyDown,
    pickItem,
    resetMenu,
  };
};
