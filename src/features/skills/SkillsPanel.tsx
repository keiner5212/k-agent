import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Folder, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { useSettingsStore } from "@/lib/settings";
import { useSkillsStore } from "@/lib/skills";
import type { SkillContext as SkillContextType } from "@/types/skills";
import { CreateSkillDialog } from "./CreateSkillDialog";
import { DeleteSkillDialog } from "./DeleteSkillDialog";
import { SkillEditorDialog } from "./SkillEditorDialog";

type Tab = "global" | "local";

export const SkillsPanel = (): ReactNode => {
  const { t } = useTranslation();
  const globalSkillsPath = useSettingsStore((state) => state.globalSkillsPath);
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
    void load(globalSkillsPath);
  }, [globalSkillsPath, load]);

  const global = contexts.find((context) => context.kind === "global");
  const local = contexts.find((context) => context.kind === "local");
  const showLocal = Boolean(workspacePath) && Boolean(local);
  const active = tab === "local" && showLocal ? local : global;
  const tabs: Array<{ id: Tab; count: number; available: boolean }> = [
    { id: "global", count: global?.skills.length ?? 0, available: Boolean(global) },
    { id: "local", count: local?.skills.length ?? 0, available: showLocal },
  ];

  const handleRefresh = (): void => {
    void refresh(globalSkillsPath);
  };

  return (
    <section className="skills-panel">
      <header className="skills-panel__head">
        <h2 className="section__heading">{t("skills.title")}</h2>
        <p className="section__description">{t("skills.description")}</p>
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
          const labelKey =
            entry.id === "global" ? "skills.context.global" : "skills.context.local";
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

      <div
        id="skills-panel-tab-panel"
        role="tabpanel"
        className="skills-panel__panel"
        key={tab}
      >
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
          await refresh(globalSkillsPath);
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
          await refresh(globalSkillsPath);
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
          await refresh(globalSkillsPath);
          return undefined;
        }}
      />
    </section>
  );
};

type SkillContextViewProps = {
  context: SkillContextType;
  onCreate: () => void;
  onEdit: (skill: { id: string; path: string }) => void;
  onDelete: (skill: { id: string; path: string }) => void;
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
        <button
          type="button"
          className="skill-context__new"
          onClick={onCreate}
          aria-label={t("skills.actions.create")}
        >
          <Plus size={12} strokeWidth={1.5} />
          <span>{t("skills.actions.create")}</span>
        </button>
      </div>
      {context.skills.length === 0 ? (
        <p className="skill-context__empty">{t("skills.empty")}</p>
      ) : (
        <ul className="skill-list">
          {context.skills.map((skill) => (
            <li key={skill.id} className="skill-row" title={skill.path}>
              <span className="skill-row__id">{skill.id}</span>
              <span className="skill-row__actions">
                <IconButton
                  label={t("skills.actions.edit")}
                  onClick={() => onEdit(skill)}
                >
                  <Pencil size={12} strokeWidth={1.5} />
                </IconButton>
                <IconButton
                  label={t("skills.actions.delete")}
                  onClick={() => onDelete(skill)}
                >
                  <Trash2 size={12} strokeWidth={1.5} />
                </IconButton>
              </span>
            </li>
          ))}
        </ul>
      )}
    </article>
  );
};

