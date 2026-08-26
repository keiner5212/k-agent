import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import {
  builtinAgentKey,
  listEnabledBuiltinAgents,
  type BuiltinAgentId,
} from "@/lib/builtin-agents";
import { listComposerAgentKeys } from "@/lib/composer-agents";
import { useAgentsStore } from "@/lib/agents";
import { hydrateWorkspaceConfig } from "@/lib/workspace-config";
import { useComposerStore } from "@/lib/composer";
import { useSettingsStore } from "@/lib/settings";
import { agentKey } from "@/types/agents";

export const AgentSelector = (): ReactNode => {
  const { t } = useTranslation();
  const selectedAgent = useComposerStore((state) => state.selectedAgent);
  const agentHydrated = useComposerStore((state) => state.agentHydrated);
  const setSelectedAgent = useComposerStore((state) => state.setSelectedAgent);
  const contexts = useAgentsStore((state) => state.contexts);
  const buildAgentEnabled = useSettingsStore((state) => state.buildAgentEnabled);
  const planAgentEnabled = useSettingsStore((state) => state.planAgentEnabled);

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
  const options = [...builtinOptions, ...userOptions];

  useEffect(() => {
    if (!agentHydrated) return;
    const values = optionValues.length === 0 ? [] : optionValues.split("|");
    if (values.length === 0) {
      if (selectedAgent !== "") setSelectedAgent("");
      return;
    }
    if (!values.includes(selectedAgent)) {
      setSelectedAgent(values[0] ?? "");
    }
  }, [agentHydrated, optionValues, selectedAgent, setSelectedAgent]);

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
