import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import { useAgentsStore } from "@/lib/agents";
import { useComposerStore } from "@/lib/composer";
import { agentKey } from "@/types/agents";

export const AgentSelector = (): ReactNode => {
  const { t } = useTranslation();
  const selectedAgent = useComposerStore((state) => state.selectedAgent);
  const setSelectedAgent = useComposerStore((state) => state.setSelectedAgent);
  const contexts = useAgentsStore((state) => state.contexts);
  const load = useAgentsStore((state) => state.load);

  useEffect(() => {
    void load();
  }, [load]);

  const options = contexts.flatMap((context) =>
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
  const optionValues = options.map((option) => option.value).join("|");

  useEffect(() => {
    const values = optionValues.length === 0 ? [] : optionValues.split("|");
    if (values.length === 0) {
      if (selectedAgent !== "") setSelectedAgent("");
      return;
    }
    if (!values.includes(selectedAgent)) {
      setSelectedAgent(values[0] ?? "");
    }
  }, [optionValues, selectedAgent, setSelectedAgent]);

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
        disabled={options.length === 0}
      />
    </div>
  );
};
