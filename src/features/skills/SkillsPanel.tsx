import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Folder, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { highlightMatch } from "@/lib/highlight";
import { useSkillsStore } from "@/lib/skills";
import { useAgentsStore } from "@/lib/agents";
import { formatContextWindow } from "@/types/providers";
import type { SkillContext as SkillContextType, SkillInfo } from "@/types/skills";
import { CreateSkillDialog } from "./CreateSkillDialog";
import { DeleteSkillDialog } from "./DeleteSkillDialog";
import { SkillEditorDialog } from "./SkillEditorDialog";

type Tab = "global" | "local";

type SkillsPanelProps = {
  query: string;
};

export const SkillsPanel = ({ query }: SkillsPanelProps): ReactNode => {
  const { t } = useTranslation();
  const contexts = useSkillsStore((state) => state.contexts);
  const workspacePath = useSkillsStore((state) => state.workspacePath);
  const loading = useSkillsStore((state) => state.loading);
  const error = useSkillsStore((state) => state.error);
  const load = useSkillsStore((state) => state.load);
  const refresh = useSkillsStore((state) => state.refresh);
  const createSkill = useSkillsStore((state) => state.create);
  const updateContent = useSkillsStore((state) => state.updateContent);
  const deleteSkill = useSkillsStore((state) => state.remove);

  const [tab, setTab] = useState<Tab>("global");
  const [editorPath, setEditorPath] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{ id: string } | null>(null);
  const [createName, setCreateName] = useState<string | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

  const global = contexts.find((context) => context.kind === "global");
  const local = contexts.find((context) => context.kind === "local");
  const showLocal = Boolean(workspacePath) && Boolean(local);
  const active = tab === "local" && showLocal ? local : global;
  const tabs: Array<{ id: Tab; count: number; available: boolean }> = [
    { id: "global", count: global?.skills.length ?? 0, available: Boolean(global) },
    { id: "local", count: local?.skills.length ?? 0, available: showLocal },
  ];

  const handleRefresh = (): void => {
    void refresh();
  };

  return (
    <section className="skills-panel">
      <header className="skills-panel__head">
        <h2 className="section__heading">{highlightMatch(t("skills.title"), query)}</h2>
        <p className="section__description">{highlightMatch(t("skills.description"), query)}</p>
        <IconButton
          className="skills-panel__refresh"
          label={t("skills.actions.refresh")}
          onClick={handleRefresh}
          disabled={loading}
        >
          <RefreshCw size={14} strokeWidth={1.5} className={loading ? "spin" : undefined} />
        </IconButton>
      </header>

      {error ? <p className="form-error">{error}</p> : null}

      <div className="skills-panel__tabs" role="tablist">
        {tabs.map((entry) => {
          const labelKey = entry.id === "global" ? "skills.context.global" : "skills.context.local";
          const isActive = entry.id === tab;
          return (
            <button
              key={entry.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              aria-controls="skills-panel-tab-panel"
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

      <div id="skills-panel-tab-panel" role="tabpanel" className="skills-panel__panel" key={tab}>
        {active ? (
          <SkillContextView
            context={active}
            onCreate={() => setCreateName(active.path)}
            onEdit={(skill) => setEditorPath(skill.path)}
            onDelete={(skill) => setDeleteTarget({ id: skill.id })}
          />
        ) : (
          <p className="skills-panel__empty">{t("skills.emptyTab")}</p>
        )}
      </div>

      <CreateSkillDialog
        open={createName !== null}
        onOpenChange={(open) => {
          if (!open) setCreateName(null);
        }}
        onSubmit={async (name, description) => {
          if (!createName) return "no root";
          const result = await createSkill(createName, name, description);
          if (result.error) return result.error;
          await refresh();
          return undefined;
        }}
      />

      <SkillEditorDialog
        open={editorPath !== null}
        skillPath={editorPath}
        onOpenChange={(open) => {
          if (!open) setEditorPath(null);
        }}
        onSave={async (content) => {
          if (!editorPath) return "no skill";
          const result = await updateContent(editorPath, content);
          if (result.error) return result.error;
          await refresh();
          return undefined;
        }}
      />

      <DeleteSkillDialog
        open={deleteTarget !== null}
        skillName={deleteTarget?.id ?? ""}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
        onConfirm={async () => {
          if (!deleteTarget || !active) return "missing target";
          const result = await deleteSkill(active.path, deleteTarget.id);
          if (result.error) return result.error;
          await refresh();
          await useAgentsStore.getState().refresh();
          return undefined;
        }}
      />
    </section>
  );
};

type SkillContextViewProps = {
  context: SkillContextType;
  onCreate: () => void;
  onEdit: (skill: SkillInfo) => void;
  onDelete: (skill: SkillInfo) => void;
};

const SkillContextView = ({
  context,
  onCreate,
  onEdit,
  onDelete,
}: SkillContextViewProps): ReactNode => {
  const { t } = useTranslation();
  return (
    <article className="skill-context">
      <div className="skill-context__header">
        <Folder size={12} strokeWidth={1.5} />
        <span className="skill-context__path" title={context.path}>
          {context.path}
        </span>
        <GlassButton
          variant="primary"
          className="skill-context__new"
          onClick={onCreate}
          aria-label={t("skills.actions.create")}
        >
          <Plus strokeWidth={1.5} />
          {t("skills.actions.create")}
        </GlassButton>
      </div>
      {context.skills.length === 0 ? (
        <p className="skill-context__empty">{t("skills.empty")}</p>
      ) : (
        <div className="skill-table-wrap">
          <table className="skill-table">
            <thead>
              <tr>
                <th>{t("skills.table.name")}</th>
                <th>{t("skills.table.description")}</th>
                <th>{t("skills.table.tokens")}</th>
                <th>
                  <span className="visually-hidden">{t("skills.table.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {context.skills.map((skill) => (
                <tr key={skill.id} title={skill.path}>
                  <td className="skill-table__name">{skill.name}</td>
                  <td
                    className="skill-table__desc"
                    data-empty={skill.description.trim() ? undefined : "true"}
                  >
                    {skill.description.trim() ? skill.description : t("skills.table.noDescription")}
                  </td>
                  <td className="skill-table__tokens">
                    {t("skills.table.tokenValue", {
                      value: formatContextWindow(skill.estimatedTokens),
                    })}
                  </td>
                  <td className="skill-table__actions">
                    <IconButton label={t("skills.actions.edit")} onClick={() => onEdit(skill)}>
                      <Pencil size={12} strokeWidth={1.5} />
                    </IconButton>
                    <IconButton label={t("skills.actions.delete")} onClick={() => onDelete(skill)}>
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
