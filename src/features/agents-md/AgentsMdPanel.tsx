import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { highlightMatch } from "@/lib/highlight";
import { useAgentsMdStore } from "@/lib/agents-md";
import { formatContextWindow } from "@/types/providers";
import type { AgentsMdFile, AgentsMdKind } from "@/types/agents-md";

type Tab = AgentsMdKind;

type AgentsMdPanelProps = {
  query: string;
};

export const AgentsMdPanel = ({ query }: AgentsMdPanelProps): ReactNode => {
  const { t } = useTranslation();
  const files = useAgentsMdStore((state) => state.files);
  const workspacePath = useAgentsMdStore((state) => state.workspacePath);
  const loading = useAgentsMdStore((state) => state.loading);
  const error = useAgentsMdStore((state) => state.error);
  const load = useAgentsMdStore((state) => state.load);
  const refresh = useAgentsMdStore((state) => state.refresh);

  const [tab, setTab] = useState<Tab>("global");

  useEffect(() => {
    void load();
  }, [load]);

  const global = files.find((file) => file.kind === "global");
  const local = files.find((file) => file.kind === "local");
  const showLocal = Boolean(workspacePath) && Boolean(local);
  const active = tab === "local" && showLocal ? local : global;
  const tabs: Array<{ id: Tab; count: number; available: boolean }> = [
    { id: "global", count: global?.exists ? 1 : 0, available: Boolean(global) },
    { id: "local", count: local?.exists ? 1 : 0, available: showLocal },
  ];

  return (
    <section className="skills-panel">
      <header className="skills-panel__head">
        <h2 className="section__heading">{highlightMatch(t("agentsMd.title"), query)}</h2>
        <p className="section__description">{highlightMatch(t("agentsMd.description"), query)}</p>
        <IconButton
          className="skills-panel__refresh"
          label={t("agentsMd.actions.refresh")}
          onClick={() => void refresh()}
          disabled={loading}
        >
          <RefreshCw size={14} strokeWidth={1.5} className={loading ? "spin" : undefined} />
        </IconButton>
      </header>

      {error ? <p className="form-error">{error}</p> : null}

      <div className="skills-panel__tabs" role="tablist">
        {tabs.map((entry) => {
          const labelKey =
            entry.id === "global" ? "agentsMd.context.global" : "agentsMd.context.local";
          const isActive = entry.id === tab;
          return (
            <button
              key={entry.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              aria-controls="agents-md-panel-tab-panel"
              disabled={!entry.available}
              className="skills-panel__tab"
              data-active={isActive ? "true" : "false"}
              onClick={() => setTab(entry.id)}
            >
              <span className="skills-panel__tab-label">{t(labelKey)}</span>
              <span className="skills-panel__tab-count">{entry.count}</span>
            </button>
          );
        })}
      </div>

      <div id="agents-md-panel-tab-panel" role="tabpanel" className="skills-panel__panel" key={tab}>
        {active ? (
          <AgentsMdFileView file={active} />
        ) : (
          <p className="skills-panel__empty">{t("agentsMd.emptyTab")}</p>
        )}
      </div>
    </section>
  );
};

const AgentsMdFileView = ({ file }: { file: AgentsMdFile }): ReactNode => {
  const { t } = useTranslation();

  return (
    <article className="skill-context">
      <div className="skill-context__header">
        <span className="skill-context__path" title={file.path}>
          {file.path}
        </span>
        <span className="skill-context__count" data-exists={file.exists ? "true" : "false"}>
          {t(file.exists ? "agentsMd.detected" : "agentsMd.missing")}
        </span>
        {file.exists ? (
          <span className="skill-context__count">
            {t("agentsMd.tokens", { value: formatContextWindow(file.estimatedTokens) })}
          </span>
        ) : null}
      </div>
      {file.exists ? (
        <pre className="agents-md__body">{file.content}</pre>
      ) : (
        <p className="skill-context__empty">{t("agentsMd.empty")}</p>
      )}
    </article>
  );
};
