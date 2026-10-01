import { useCallback, useEffect, useMemo, useState } from "react";
import {
  filterSlashCommands,
  parseActiveSlashCommand,
  SLASH_COMMAND_NAMES,
  SLASH_COMMAND_RESULT_LIMIT,
  type SlashCommandDef,
} from "@/lib/slash-commands";
import {
  applySlashToken,
  buildSkillNameSet,
  filterSkills,
  flattenSkills,
} from "@/lib/skill-mentions";
import { hydrateWorkspaceConfig } from "@/lib/workspace-config";
import { useSkillsStore } from "@/lib/skills";
import type { SkillInfo } from "@/types/skills";

export type SlashPickItem =
  { kind: "command"; command: SlashCommandDef } | { kind: "skill"; skill: SkillInfo };

type UseSlashCommandsArgs = {
  value: string;
  cursor: number;
  enabled?: boolean;
  onApply: (next: string, cursor: number) => void;
};

type UseSlashCommandsResult = {
  menuOpen: boolean;
  items: SlashPickItem[];
  tooMany: boolean;
  activeIndex: number;
  query: string;
  slashNames: Set<string>;
  loading: boolean;
  error?: string;
  handleKeyDown: (event: React.KeyboardEvent<HTMLTextAreaElement>) => boolean;
  pickItem: (item: SlashPickItem) => void;
};

type SlashSelection = {
  key: string;
  index: number;
};

export const useSlashCommands = ({
  value,
  cursor,
  enabled = true,
  onApply,
}: UseSlashCommandsArgs): UseSlashCommandsResult => {
  const skillContexts = useSkillsStore((state) => state.contexts);
  const skillsError = useSkillsStore((state) => state.error);
  const skillsLoading = useSkillsStore((state) => state.loading);
  const [selection, setSelection] = useState<SlashSelection>({ key: "", index: 0 });
  const [dismissedKey, setDismissedKey] = useState<string | null>(null);

  const activeSlash = parseActiveSlashCommand(value, cursor);
  const slashKey = activeSlash ? `${activeSlash.start}:${activeSlash.query}` : "";
  const menuOpen = enabled && slashKey.length > 0 && dismissedKey !== slashKey;
  const activeIndex = selection.key === slashKey ? selection.index : 0;

  const allSkills = useMemo(() => flattenSkills(skillContexts), [skillContexts]);
  const commandFilter = useMemo(
    () => filterSlashCommands(activeSlash?.query ?? ""),
    [activeSlash?.query],
  );
  const skillFilter = useMemo(
    () => filterSkills(allSkills, activeSlash?.query ?? ""),
    [allSkills, activeSlash?.query],
  );

  const items = useMemo((): SlashPickItem[] => {
    const out: SlashPickItem[] = [];
    for (const command of commandFilter) {
      out.push({ kind: "command", command });
      if (out.length >= SLASH_COMMAND_RESULT_LIMIT) return out;
    }
    for (const skill of skillFilter.items) {
      out.push({ kind: "skill", skill });
      if (out.length >= SLASH_COMMAND_RESULT_LIMIT) return out;
    }
    return out;
  }, [commandFilter, skillFilter.items]);

  const tooMany =
    skillFilter.tooMany ||
    commandFilter.length + skillFilter.items.length > SLASH_COMMAND_RESULT_LIMIT;

  const skillNames = useMemo(() => buildSkillNameSet(allSkills), [allSkills]);
  const slashNames = useMemo(() => {
    const out = new Set<string>(SLASH_COMMAND_NAMES);
    for (const name of skillNames) out.add(name);
    return out;
  }, [skillNames]);

  useEffect(() => {
    if (!enabled) return;
    void hydrateWorkspaceConfig();
  }, [enabled]);

  const setActiveIndex = useCallback(
    (next: number | ((prev: number) => number)) => {
      setSelection((prev) => {
        const base = prev.key === slashKey ? prev.index : 0;
        const index = typeof next === "function" ? next(base) : next;
        return { key: slashKey, index };
      });
    },
    [slashKey],
  );

  const pickItem = useCallback(
    (item: SlashPickItem) => {
      if (!activeSlash) return;
      const picked =
        item.kind === "command"
          ? applySlashToken(value, activeSlash, item.command.name, item.command.estimatedTokens)
          : applySlashToken(value, activeSlash, item.skill.name, item.skill.estimatedTokens);
      onApply(picked.next, picked.cursor);
      setSelection({ key: "", index: 0 });
      setDismissedKey(null);
    },
    [activeSlash, onApply, value],
  );

  const resetMenu = useCallback(() => {
    setDismissedKey(slashKey);
    setSelection({ key: slashKey, index: 0 });
  }, [slashKey]);

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
        const item = items[activeIndex];
        if (item) pickItem(item);
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
    query: activeSlash?.query ?? "",
    slashNames,
    loading: skillsLoading,
    error: skillsError,
    handleKeyDown,
    pickItem,
  };
};
