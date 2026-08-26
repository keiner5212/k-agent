import type { TFunction } from "i18next";
import i18n from "@/i18n";
import { useAgentsStore } from "@/lib/agents";
import {
  builtinAgentKey,
  listEnabledBuiltinAgents,
  type BuiltinAgentId,
} from "@/lib/builtin-agents";
import { matchesChordString } from "@/lib/keybindings";
import { useComposerStore } from "@/lib/composer";
import { useSettingsStore } from "@/lib/settings";
import { agentKey, type AgentContext } from "@/types/agents";

export type ComposerAgentToggles = {
  build: boolean;
  plan: boolean;
};

export const listComposerAgentKeys = (
  t: TFunction,
  contexts: AgentContext[],
  enabled: ComposerAgentToggles,
): string[] => {
  const builtin = listEnabledBuiltinAgents(t, enabled).map((agent) =>
    builtinAgentKey(agent.id as BuiltinAgentId),
  );
  const user = contexts
    .filter((context) => context.kind === "global")
    .flatMap((context) => context.agents.map((agent) => agentKey("global", agent.id)));
  return [...builtin, ...user];
};

export const cycleComposerAgent = (
  current: string,
  keys: readonly string[],
  direction: 1 | -1,
): string => {
  if (keys.length === 0) return "";
  if (keys.length === 1) return keys[0] ?? "";
  const index = keys.indexOf(current);
  const base = index < 0 ? 0 : index;
  const next = (base + direction + keys.length) % keys.length;
  return keys[next] ?? keys[0] ?? "";
};

export const applyComposerAgentCycle = (direction: 1 | -1): boolean => {
  const { mode, selectedAgent, setSelectedAgent } = useComposerStore.getState();
  if (mode === "shell") return false;
  const { buildAgentEnabled, planAgentEnabled } = useSettingsStore.getState();
  const keys = listComposerAgentKeys(i18n.t.bind(i18n), useAgentsStore.getState().contexts, {
    build: buildAgentEnabled,
    plan: planAgentEnabled,
  });
  if (keys.length === 0) return false;
  setSelectedAgent(cycleComposerAgent(selectedAgent, keys, direction));
  return true;
};

export const matchesAgentCycle = (event: KeyboardEvent, binding: string): boolean =>
  matchesChordString(event, binding);
