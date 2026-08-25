import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { Toggle } from "@/components/Toggle";
import type { AgentWriteInput } from "@/lib/agents";
import {
  AGENT_TOOL_IDS,
  MAX_AGENT_SKILLS,
  skillRefKey,
  type AgentContextKind,
  type AgentMeta,
  type AgentSkillRef,
} from "@/types/agents";

type AgentFormDialogProps = {
  open: boolean;
  mode: "create" | "edit";
  kind: AgentContextKind;
  availableSkills: AgentSkillRef[];
  initial?: AgentMeta | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (input: AgentWriteInput) => Promise<string | undefined>;
};

export const AgentFormDialog = ({
  open,
  mode,
  kind,
  availableSkills,
  initial,
  onOpenChange,
  onSubmit,
}: AgentFormDialogProps): ReactNode => {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey={mode === "create" ? "agents.form.createTitle" : "agents.form.editTitle"}
      size="narrow"
      placement="center"
    >
      {open ? (
        <AgentFormBody
          key={initial?.path ?? `${kind}-create`}
          kind={kind}
          availableSkills={availableSkills}
          initial={initial}
          onOpenChange={onOpenChange}
          onSubmit={onSubmit}
        />
      ) : null}
    </Dialog>
  );
};

type AgentFormBodyProps = {
  kind: AgentContextKind;
  availableSkills: AgentSkillRef[];
  initial?: AgentMeta | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (input: AgentWriteInput) => Promise<string | undefined>;
};

const AgentFormBody = ({
  kind,
  availableSkills,
  initial,
  onOpenChange,
  onSubmit,
}: AgentFormBodyProps): ReactNode => {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.id ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");
  const [skills, setSkills] = useState<AgentSkillRef[]>(initial?.skills ?? []);
  const [tools, setTools] = useState<string[]>(initial?.tools ?? []);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const atSkillCap = skills.length >= MAX_AGENT_SKILLS;

  const toggleSkill = (skill: AgentSkillRef, checked: boolean): void => {
    if (checked) {
      if (atSkillCap) return;
      if (skills.some((item) => skillRefKey(item) === skillRefKey(skill))) return;
      setSkills([...skills, skill]);
      return;
    }
    setSkills(skills.filter((item) => skillRefKey(item) !== skillRefKey(skill)));
  };

  const toggleTool = (tool: string, checked: boolean): void => {
    if (checked) {
      if (tools.includes(tool)) return;
      setTools([...tools, tool]);
      return;
    }
    setTools(tools.filter((item) => item !== tool));
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>): Promise<void> => {
    event.preventDefault();
    if (!name.trim()) {
      setError(t("agents.form.errors.nameRequired"));
      return;
    }
    setSubmitting(true);
    setError(null);
    const saveError = await onSubmit({
      name: name.trim(),
      description: description.trim(),
      skills,
      tools,
    });
    setSubmitting(false);
    if (saveError) {
      setError(saveError);
      return;
    }
    onOpenChange(false);
  };

  return (
    <form className="skill-form agent-form" onSubmit={(event) => void handleSubmit(event)}>
      <div className="field">
        <label className="field__label" htmlFor="agent-name">
          {t("agents.form.name")}
        </label>
        <input
          id="agent-name"
          className="input input--mono"
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder={t("agents.form.namePlaceholder")}
          autoComplete="off"
          spellCheck={false}
          pattern="[a-z0-9][a-z0-9_\-]*"
          title={t("agents.form.nameHint")}
          required
        />
        <span className="field__hint">{t("agents.form.nameHint")}</span>
      </div>
      <div className="field">
        <label className="field__label" htmlFor="agent-description">
          {t("agents.form.description")}
        </label>
        <textarea
          id="agent-description"
          className="input skill-form__textarea"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          placeholder={t("agents.form.descriptionPlaceholder")}
          rows={3}
          autoComplete="off"
        />
        <span className="field__hint">{t("agents.form.descriptionHint")}</span>
      </div>
      <fieldset className="field agent-form__fieldset">
        <legend className="field__label">
          {t("agents.form.skills")}
          <span className="agent-form__count">
            {t("agents.form.skillsCount", { count: skills.length, max: MAX_AGENT_SKILLS })}
          </span>
        </legend>
        <span className="field__hint">
          {kind === "global" ? t("agents.form.skillsHintGlobal") : t("agents.form.skillsHintLocal")}
        </span>
        {availableSkills.length === 0 ? (
          <p className="agent-form__empty">{t("agents.form.skillsEmpty")}</p>
        ) : (
          <ul className="agent-pick">
            {availableSkills.map((skill) => {
              const key = skillRefKey(skill);
              const checked = skills.some((item) => skillRefKey(item) === key);
              const disabled = !checked && atSkillCap;
              return (
                <li key={key}>
                  <label className="agent-pick__row" data-disabled={disabled ? "true" : "false"}>
                    <input
                      type="checkbox"
                      checked={checked}
                      disabled={disabled}
                      onChange={(event) => toggleSkill(skill, event.target.checked)}
                    />
                    <span className="agent-pick__id">{skill.id}</span>
                    <span className="agent-pick__kind">
                      {skill.kind === "global"
                        ? t("agents.context.global")
                        : t("agents.context.local")}
                    </span>
                  </label>
                </li>
              );
            })}
          </ul>
        )}
      </fieldset>
      <fieldset className="field agent-form__fieldset">
        <legend className="field__label">{t("agents.form.tools")}</legend>
        <span className="field__hint">{t("agents.form.toolsHint")}</span>
        <div className="agent-form__tools">
          {AGENT_TOOL_IDS.map((tool) => (
            <Toggle
              key={tool}
              checked={tools.includes(tool)}
              onChange={(next) => toggleTool(tool, next)}
              label={t(`agents.tools.${tool}.label`)}
              description={t(`agents.tools.${tool}.description`)}
            />
          ))}
        </div>
      </fieldset>
      {error ? (
        <div className="form-error" role="alert">
          {error}
        </div>
      ) : null}
      <div className="form-actions">
        <GlassButton variant="ghost" onClick={() => onOpenChange(false)} disabled={submitting}>
          {t("agents.form.cancel")}
        </GlassButton>
        <GlassButton variant="primary" type="submit" disabled={submitting || !name.trim()}>
          {submitting ? (
            <>
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
              <span>{t("agents.form.saving")}</span>
            </>
          ) : (
            <span>{t("agents.form.save")}</span>
          )}
        </GlassButton>
      </div>
    </form>
  );
};
