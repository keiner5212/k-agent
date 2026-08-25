import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import { useComposerStore } from "@/lib/composer";
import { AGENT_MODES } from "@/types/chat";

export const AgentSelector = (): ReactNode => {
  const { t } = useTranslation();
  const agentMode = useComposerStore((state) => state.agentMode);
  const setAgentMode = useComposerStore((state) => state.setAgentMode);

  return (
    <div className="agent-selector">
      <Select
        id="chat-agent-mode"
        value={agentMode}
        onChange={setAgentMode}
        options={AGENT_MODES.map((mode) => ({
          value: mode,
          label: t(`chat.agent.options.${mode}`),
        }))}
        ariaLabel={t("chat.agent.label")}
      />
    </div>
  );
};
