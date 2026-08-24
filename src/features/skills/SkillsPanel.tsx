import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { useSettingsStore } from "@/lib/settings";
import { useSkillsStore } from "@/lib/skills";
import type { SkillContext as SkillContextType } from "@/types/skills";

export const SkillsPanel = (): ReactNode => {
  const { t } = useTranslation();
  const globalSkillsPath = useSettingsStore((state) => state.globalSkillsPath);
  const contexts = useSkillsStore((state) => state.contexts);
  const workspacePath = useSkillsStore((state) => state.workspacePath);
  const loading = useSkillsStore((state) => state.loading);
  const error = useSkillsStore((state) => state.error);
  const load = useSkillsStore((state) => state.load);
  const refresh = useSkillsStore((state) => state.refresh);

  useEffect(() => {
    void load(globalSkillsPath);
  }, [globalSkillsPath, load]);

  const global = contexts.find((context) => context.kind === "global");
  const local = contexts.find((context) => context.kind === "local");
  const showLocal = Boolean(workspacePath);

  return (
    <section className="skills-panel">
      <header className="skills-panel__head">
        <h2 className="section__heading">{t("skills.title")}</h2>
        <p className="section__description">{t("skills.description")}</p>
        <IconButton
          className="skills-panel__refresh"
          label={t("skills.actions.refresh")}
          onClick={() => void refresh(globalSkillsPath)}
          disabled={loading}
        >
          <RefreshCw size={14} strokeWidth={1.5} className={loading ? "spin" : undefined} />
        </IconButton>
      </header>

      {error ? <p className="form-error">{error}</p> : null}
      {global ? <SkillContextCard context={global} /> : null}
      {showLocal && local ? <SkillContextCard context={local} /> : null}
    </section>
  );
};

const SkillContextCard = ({ context }: { context: SkillContextType }): ReactNode => {
  const { t } = useTranslation();
  const labelKey = context.kind === "global" ? "skills.context.global" : "skills.context.local";

  return (
    <article className="skill-context">
      <div className="skill-context__header">
        <span className="skill-context__label">{t(labelKey)}</span>
        <span className="skill-context__path" title={context.path}>
          {context.path}
        </span>
        <span className="skill-context__count">
          {t("skills.count", { count: context.skills.length })}
        </span>
      </div>
      {context.skills.length === 0 ? (
        <p className="skill-context__empty">{t("skills.empty")}</p>
      ) : (
        <ul className="skill-list">
          {context.skills.map((skill) => (
            <li key={skill.id} className="skill-row" title={skill.path}>
              {skill.id}
            </li>
          ))}
        </ul>
      )}
    </article>
  );
};
