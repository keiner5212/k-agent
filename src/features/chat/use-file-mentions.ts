import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyMentionSelection,
  buildMentionPathSet,
  filterDirEntries,
  mentionListingContext,
  parseActiveMention,
  type ActiveMention,
} from "@/lib/file-mentions";
import {
  useWorkspaceFilesStore,
  hasDirInCache,
  workspaceFilesLoading,
  type CachedDir,
  type WorkspaceFilesStore,
} from "@/lib/workspace-files";
import type { WorkspaceEntry } from "@/types/workspace-files";

type UseFileMentionsArgs = {
  value: string;
  cursor: number;
  enabled?: boolean;
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

const scheduleIdle = (run: () => void): (() => void) => {
  if (typeof requestIdleCallback === "function") {
    const id = requestIdleCallback(run);
    return () => cancelIdleCallback(id);
  }
  const id = window.setTimeout(run, 0);
  return () => window.clearTimeout(id);
};

const collectWorkspaceEntries = (dirs: Record<string, CachedDir>): WorkspaceEntry[] => {
  const entries: WorkspaceEntry[] = [];
  for (const key of Object.keys(dirs)) {
    const cached = dirs[key];
    if (cached) entries.push(...cached.entries);
  }
  return entries;
};

export const useFileMentions = ({
  value,
  cursor,
  enabled = true,
  onApply,
}: UseFileMentionsArgs): UseFileMentionsResult => {
  const dirs = useWorkspaceFilesStore((state: WorkspaceFilesStore) => state.dirs);
  const error = useWorkspaceFilesStore((state: WorkspaceFilesStore) => state.error);
  const ensureRootLoaded = useWorkspaceFilesStore(
    (state: WorkspaceFilesStore) => state.ensureRootLoaded,
  );
  const ensureMentionScope = useWorkspaceFilesStore(
    (state: WorkspaceFilesStore) => state.ensureMentionScope,
  );
  const prefetchDir = useWorkspaceFilesStore((state: WorkspaceFilesStore) => state.prefetchDir);
  const [selection, setSelection] = useState<MentionSelection>({ key: "", index: 0 });
  const [dismissedKey, setDismissedKey] = useState<string | null>(null);

  const activeMention = useMemo(() => parseActiveMention(value, cursor), [value, cursor]);
  const mentionKey = activeMention ? `${activeMention.start}:${activeMention.query}` : "";
  const menuOpen = enabled && mentionKey.length > 0 && dismissedKey !== mentionKey;
  const listing = useMemo(
    () => mentionListingContext(activeMention?.query ?? "", (path) => hasDirInCache(dirs, path)),
    [activeMention?.query, dirs],
  );
  const loading = useWorkspaceFilesStore((state: WorkspaceFilesStore): boolean =>
    workspaceFilesLoading(state, listing.parentDir),
  );
  const activeIndex = selection.key === mentionKey ? selection.index : 0;

  const filterResult = useMemo(() => {
    const scoped = dirs[listing.parentDir]?.entries ?? [];
    return filterDirEntries(scoped, listing.prefix);
  }, [dirs, listing.parentDir, listing.prefix]);
  const items = filterResult.items;
  const tooMany = filterResult.tooMany;

  const mentionPaths = useMemo(() => {
    if (!value.includes("@")) return new Set<string>();
    return buildMentionPathSet(collectWorkspaceEntries(dirs));
  }, [dirs, value]);

  useEffect(() => {
    if (!enabled) return;
    void ensureRootLoaded();
  }, [enabled, ensureRootLoaded]);

  useEffect(() => {
    if (!enabled || !activeMention) return;
    void ensureMentionScope(activeMention.query);
  }, [activeMention, enabled, ensureMentionScope]);

  useEffect(() => {
    if (!menuOpen) return;
    const entry = items[activeIndex];
    if (!entry || entry.kind !== "dir") return;
    return scheduleIdle(() => prefetchDir(entry.path));
  }, [activeIndex, items, menuOpen, prefetchDir]);

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
      if (!enabled || !menuOpen || tooMany) return false;
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
    [activeIndex, enabled, items, menuOpen, pickItem, resetMenu, setActiveIndex, tooMany],
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
