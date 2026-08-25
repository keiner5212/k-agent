import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Folder, FileText, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { highlightMatch } from "@/lib/highlight";
import { useAgentsStore } from "@/lib/agents";
import { useSkillsStore } from "@/lib/skills";
import { formatContextWindow } from "@/types/providers";
import type {
  AgentContext as AgentContextType,
  AgentContextKind,
  AgentMeta,
  AgentSkillRef,
} from "@/types/agents";
import { AgentFormDialog } from "./AgentFormDialog";
import { AgentPersonalityDialog } from "./AgentPersonalityDialog";
import { DeleteAgentDialog } from "./DeleteAgentDialog";

type Tab = AgentContextKind;

type AgentsPanelProps = {
  query: string;
};

export const AgentsPanel = ({ query }: AgentsPanelProps): ReactNode => {
  const { t } = useTranslation();
  const contexts = useAgentsStore((state) => state.contexts);
  const workspacePath = useAgentsStore((state) => state.workspacePath);
  const loading = useAgentsStore((state) => state.loading);
  const error = useAgentsStore((state) => state.error);
  const load = useAgentsStore((state) => state.load);
  const refresh = useAgentsStore((state) => state.refresh);
  const createAgent = useAgentsStore((state) => state.create);
  const updateAgent = useAgentsStore((state) => state.update);
  const deleteAgent = useAgentsStore((state) => state.remove);
  const skillContexts = useSkillsStore((state) => state.contexts);
  const loadSkills = useSkillsStore((state) => state.load);

  const [tab, setTab] = useState<Tab>("global");
  const [createOpen, setCreateOpen] = useState(false);
  const [editTarget, setEditTarget] = useState<AgentMeta | null>(null);
  const [personalityTarget, setPersonalityTarget] = useState<AgentMeta | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{ id: string } | null>(null);

  useEffect(() => {
    void load();
    void loadSkills();
  }, [load, loadSkills]);

  const global = contexts.find((context) => context.kind === "global");
  const local = contexts.find((context) => context.kind === "local");
  const showLocal = Boolean(workspacePath) && Boolean(local);
  const active = tab === "local" && showLocal ? local : global;
  const formKind: AgentContextKind = tab === "local" && showLocal ? "local" : "global";
  const tabs: Array<{ id: Tab; count: number; available: boolean }> = [
    { id: "global", count: global?.agents.length ?? 0, available: Boolean(global) },
    { id: "local", count: local?.agents.length ?? 0, available: showLocal },
  ];

  const availableSkills: AgentSkillRef[] = [];
  const globalSkills = skillContexts.find((context) => context.kind === "global");
  const localSkills = skillContexts.find((context) => context.kind === "local");
  for (const skill of globalSkills?.skills ?? []) {
    availableSkills.push({ kind: "global", id: skill.id });
  }
  if (formKind === "local") {
    for (const skill of localSkills?.skills ?? []) {
      availableSkills.push({ kind: "local", id: skill.id });
    }
  }

  return (
    <section className="skills-panel">
      <header className="skills-panel__head">
        <h2 className="section__heading">{highlightMatch(t("agents.title"), query)}</h2>
        <p className="section__description">{highlightMatch(t("agents.description"), query)}</p>
        <IconButton
          className="skills-panel__refresh"
          label={t("agents.actions.refresh")}
          onClick={() => void refresh()}
          disabled={loading}
        >
          <RefreshCw size={14} strokeWidth={1.5} className={loading ? "spin" : undefined} />
        </IconButton>
      </header>

      {error ? <p className="form-error">{error}</p> : null}

      <div className="skills-panel__tabs" role="tablist">
        {tabs.map((entry) => {
          const labelKey = entry.id === "global" ? "agents.context.global" : "agents.context.local";
          const isActive = entry.id === tab;
          return (
            <button
              key={entry.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              aria-controls="agents-panel-tab-panel"
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

      <div id="agents-panel-tab-panel" role="tabpanel" className="skills-panel__panel" key={tab}>
        {active ? (
          <AgentContextView
            context={active}
            onCreate={() => setCreateOpen(true)}
            onEdit={setEditTarget}
            onEditPersonality={setPersonalityTarget}
            onDelete={(agent) => setDeleteTarget({ id: agent.id })}
          />
        ) : (
          <p className="skills-panel__empty">{t("agents.emptyTab")}</p>
        )}
      </div>

      <AgentFormDialog
        open={createOpen}
        mode="create"
        kind={formKind}
        availableSkills={availableSkills}
        onOpenChange={setCreateOpen}
        onSubmit={async (input) => {
          if (!active) return "no root";
          const result = await createAgent(active.path, formKind, input);
          if (result.error) return result.error;
          await refresh();
          return undefined;
        }}
      />

      <AgentFormDialog
        open={editTarget !== null}
        mode="edit"
        kind={formKind}
        availableSkills={availableSkills}
        initial={editTarget}
        onOpenChange={(open) => {
          if (!open) setEditTarget(null);
        }}
        onSubmit={async (input) => {
          if (!editTarget) return "no agent";
          const result = await updateAgent(editTarget.path, formKind, input);
          if (result.error) return result.error;
          await refresh();
          return undefined;
        }}
      />

      <AgentPersonalityDialog
        open={personalityTarget !== null}
        agent={personalityTarget}
        onOpenChange={(open) => {
          if (!open) setPersonalityTarget(null);
        }}
        onSave={async (personality) => {
          if (!personalityTarget) return "no agent";
          const result = await updateAgent(personalityTarget.path, formKind, {
            name: personalityTarget.id,
            description: personalityTarget.description,
            personality,
            skills: personalityTarget.skills,
            tools: personalityTarget.tools,
          });
          if (result.error) return result.error;
          await refresh();
          return undefined;
        }}
      />

      <DeleteAgentDialog
        open={deleteTarget !== null}
        agentName={deleteTarget?.id ?? ""}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        onConfirm={async () => {
          if (!deleteTarget || !active) return "missing target";
          const result = await deleteAgent(active.path, deleteTarget.id);
          if (result.error) return result.error;
          await refresh();
          return undefined;
        }}
      />
    </section>
  );
};

type AgentContextViewProps = {
  context: AgentContextType;
  onCreate: () => void;
  onEdit: (agent: AgentMeta) => void;
  onEditPersonality: (agent: AgentMeta) => void;
  onDelete: (agent: AgentMeta) => void;
};

const AgentContextView = ({
  context,
  onCreate,
  onEdit,
  onEditPersonality,
  onDelete,
}: AgentContextViewProps): ReactNode => {
  const { t } = useTranslation();
  return (
    <article className="skill-context">
      <div className="skill-context__header">
        <Folder size={12} strokeWidth={1.5} />
        <span className="skill-context__path" title={context.path}>
          {context.path}
        </span>
        <GlassButton
          variant="ghost"
          className="skill-context__new"
          onClick={onCreate}
          aria-label={t("agents.actions.create")}
        >
          <Plus strokeWidth={1.5} />
          {t("agents.actions.create")}
        </GlassButton>
      </div>
      {context.agents.length === 0 ? (
        <p className="skill-context__empty">{t("agents.empty")}</p>
      ) : (
        <div className="skill-table-wrap">
          <table className="skill-table">
            <thead>
              <tr>
                <th>{t("agents.table.name")}</th>
                <th>{t("agents.table.description")}</th>
                <th>{t("agents.table.tokens")}</th>
                <th>
                  <span className="visually-hidden">{t("agents.table.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {context.agents.map((agent) => (
                <tr key={agent.id} title={agent.path}>
                  <td className="skill-table__name">{agent.name}</td>
                  <td
                    className="skill-table__desc"
                    data-empty={agent.description.trim() ? undefined : "true"}
                  >
                    {agent.description.trim() ? agent.description : t("agents.table.noDescription")}
                  </td>
                  <td className="skill-table__tokens">
                    {t("agents.table.tokenValue", {
                      value: formatContextWindow(agent.estimatedTokens),
                    })}
                  </td>
                  <td className="skill-table__actions">
                    <IconButton
                      label={t("agents.actions.editPersonality")}
                      onClick={() => onEditPersonality(agent)}
                    >
                      <FileText size={12} strokeWidth={1.5} />
                    </IconButton>
                    <IconButton label={t("agents.actions.edit")} onClick={() => onEdit(agent)}>
                      <Pencil size={12} strokeWidth={1.5} />
                    </IconButton>
                    <IconButton label={t("agents.actions.delete")} onClick={() => onDelete(agent)}>
                      <Trash2 size={12} strokeWidth={1.5} />
                    </IconButton>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </article>
  );
};
