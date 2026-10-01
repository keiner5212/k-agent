import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import {
  builtinAgentKey,
  listEnabledBuiltinAgents,
  type BuiltinAgentId,
} from "@/lib/builtin-agents";
import { listComposerAgentKeys, resolveDefaultComposerAgent } from "@/lib/composer-agents";
import { useAgentsStore } from "@/lib/agents";
import { hydrateWorkspaceConfig } from "@/lib/workspace-config";
import { useComposerStore } from "@/lib/composer";
import { useSettingsStore } from "@/lib/settings";
import { agentKey } from "@/types/agents";

export const AgentSelector = (): ReactNode => {
  const { t } = useTranslation();
  const selectedAgent = useComposerStore((state) => state.selectedAgent);
  const setSelectedAgent = useComposerStore((state) => state.setSelectedAgent);
  const contexts = useAgentsStore((state) => state.contexts);
  const agentsHydrated = useAgentsStore((state) => state.hydrated);
  const agentsError = useAgentsStore((state) => state.error);
  const settingsHydrated = useSettingsStore((state) => state.hydrated);
  const buildAgentEnabled = useSettingsStore((state) => state.buildAgentEnabled);
  const planAgentEnabled = useSettingsStore((state) => state.planAgentEnabled);
  const defaultAgent = useSettingsStore((state) => state.defaultAgent);
  const setBuildAgentEnabled = useSettingsStore((state) => state.setBuildAgentEnabled);
  const setPlanAgentEnabled = useSettingsStore((state) => state.setPlanAgentEnabled);
  const setDefaultAgent = useSettingsStore((state) => state.setDefaultAgent);

  useEffect(() => {
    void hydrateWorkspaceConfig();
  }, []);

  const builtinAgents = listEnabledBuiltinAgents(t, {
    build: buildAgentEnabled,
    plan: planAgentEnabled,
  });

  const userOptions = contexts.flatMap((context) =>
    context.agents.map((agent) => ({
      value: agentKey(context.kind, agent.id),
      label: (
        <span className="agent-option">
          <span className="agent-option__name">{agent.name}</span>
          {agent.description ? (
            <span className="agent-option__desc">{agent.description}</span>
          ) : null}
        </span>
      ),
    })),
  );

  const builtinOptions = builtinAgents.map((agent) => ({
    value: builtinAgentKey(agent.id as BuiltinAgentId),
    label: (
      <span className="agent-option">
        <span className="agent-option__name">{agent.name}</span>
        {agent.description ? <span className="agent-option__desc">{agent.description}</span> : null}
      </span>
    ),
  }));

  const optionValues = listComposerAgentKeys(t, contexts, {
    build: buildAgentEnabled,
    plan: planAgentEnabled,
  }).join("|");
  const userAgentCount = contexts.reduce(
    (count, context) => count + (context.kind === "builtin" ? 0 : context.agents.length),
    0,
  );
  const options = [...builtinOptions, ...userOptions];

  useEffect(() => {
    if (!settingsHydrated || !agentsHydrated) return;
    if (!agentsError && !buildAgentEnabled && !planAgentEnabled && userAgentCount === 0) {
      const build = builtinAgentKey("build");
      setBuildAgentEnabled(true);
      setPlanAgentEnabled(true);
      setDefaultAgent(build);
      setSelectedAgent(build);
      return;
    }
    const keys = optionValues.length === 0 ? [] : optionValues.split("|");
    const next = resolveDefaultComposerAgent(defaultAgent, keys);
    if (useComposerStore.getState().selectedAgent !== next) setSelectedAgent(next);
  }, [
    agentsError,
    agentsHydrated,
    buildAgentEnabled,
    defaultAgent,
    optionValues,
    planAgentEnabled,
    setBuildAgentEnabled,
    setDefaultAgent,
    setPlanAgentEnabled,
    setSelectedAgent,
    settingsHydrated,
    userAgentCount,
  ]);

  return (
    <div className="agent-selector">
      <Select
        id="chat-agent"
        value={selectedAgent}
        onChange={setSelectedAgent}
        options={options}
        placeholder={t("chat.agent.empty")}
        ariaLabel={t("chat.agent.label")}
        placement="up"
        menuMinWidth={240}
        virtualize={false}
        disabled={options.length === 0}
      />
    </div>
  );
};
