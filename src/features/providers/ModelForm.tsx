import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { Toggle } from "@/components/Toggle";
import {
  formatContextWindow,
  parseTokenAmount,
  type ModelDraft,
  type ModelInfo,
} from "@/types/providers";

type ModelFormProps = {
  model?: ModelInfo;
  familyPreset?: string;
  onCancel: () => void;
  onSave: (draft: ModelDraft) => Promise<string | undefined>;
  onDelete?: () => Promise<string | undefined>;
};

const tokenField = (value: number | undefined): string => {
  if (value === undefined) return "";
  return formatContextWindow(value) === "-" ? String(value) : formatContextWindow(value);
};

export const ModelForm = ({
  model,
  familyPreset,
  onCancel,
  onSave,
  onDelete,
}: ModelFormProps): ReactNode => {
  const { t } = useTranslation();
  const [id, setId] = useState(model?.id ?? "");
  const [displayName, setDisplayName] = useState(model?.displayName ?? "");
  const [family, setFamily] = useState(model?.family ?? familyPreset ?? "");
  const [contextRaw, setContextRaw] = useState(tokenField(model?.contextWindow));
  const [outputRaw, setOutputRaw] = useState(tokenField(model?.maxOutputTokens));
  const [multimodal, setMultimodal] = useState(model?.multimodal ?? false);
  const [effortLevelsRaw, setEffortLevelsRaw] = useState((model?.effortLevels ?? []).join(", "));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isEditing = Boolean(model);

  const parseOptionalTokens = (raw: string, label: string): number | undefined | "invalid" => {
    if (!raw.trim()) return undefined;
    const parsed = parseTokenAmount(raw);
    if (parsed === undefined) {
      setError(t("providers.modelForm.errors.tokens", { field: label }));
      return "invalid";
    }
    return parsed;
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>): Promise<void> => {
    event.preventDefault();
    if (!id.trim()) {
      setError(t("providers.modelForm.errors.required"));
      return;
    }
    const contextWindow = parseOptionalTokens(contextRaw, t("providers.model.context"));
    if (contextWindow === "invalid") return;
    const maxOutputTokens = parseOptionalTokens(outputRaw, t("providers.model.output"));
    if (maxOutputTokens === "invalid") return;

    setSubmitting(true);
    setError(null);
    const effortLevels = effortLevelsRaw
      .split(",")
      .map((level) => level.trim())
      .filter(Boolean);
    const saveError = await onSave({
      originalId: model?.id,
      id: id.trim(),
      displayName: displayName.trim() || undefined,
      family: family.trim() || undefined,
      contextWindow,
      maxOutputTokens,
      multimodal,
      effortLevels,
    });
    setSubmitting(false);
    if (saveError) setError(saveError);
  };

  const handleDelete = async (): Promise<void> => {
    if (!onDelete) return;
    setSubmitting(true);
    setError(null);
    const deleteError = await onDelete();
    setSubmitting(false);
    if (deleteError) setError(deleteError);
  };

  return (
    <form className="model-form" onSubmit={(event) => void handleSubmit(event)}>
      <div className="field">
        <label className="field__label" htmlFor="model-id">
          {t("providers.modelForm.id")}
        </label>
        <input
          id="model-id"
          className="input input--mono"
          value={id}
          onChange={(event) => setId(event.target.value)}
          autoComplete="off"
          spellCheck={false}
          required
        />
      </div>
      <div className="field">
        <label className="field__label" htmlFor="model-name">
          {t("providers.modelForm.displayName")}
        </label>
        <input
          id="model-name"
          className="input"
          value={displayName}
          onChange={(event) => setDisplayName(event.target.value)}
          autoComplete="off"
        />
      </div>
      <div className="field">
        <label className="field__label" htmlFor="model-family">
          {t("providers.modelForm.family")}
        </label>
        <input
          id="model-family"
          className="input"
          value={family}
          onChange={(event) => setFamily(event.target.value)}
          autoComplete="off"
          spellCheck={false}
        />
        <span className="field__hint">{t("providers.modelForm.familyHint")}</span>
      </div>
      <div className="model-form__row">
        <div className="field">
          <label className="field__label" htmlFor="model-context">
            {t("providers.model.context")}
          </label>
          <input
            id="model-context"
            className="input input--mono"
            value={contextRaw}
            onChange={(event) => setContextRaw(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            placeholder="512k"
          />
        </div>
        <div className="field">
          <label className="field__label" htmlFor="model-output">
            {t("providers.model.output")}
          </label>
          <input
            id="model-output"
            className="input input--mono"
            value={outputRaw}
            onChange={(event) => setOutputRaw(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            placeholder="64k"
          />
        </div>
      </div>
      <p className="field__hint">{t("providers.modelForm.tokenHint")}</p>
      <Toggle
        checked={multimodal}
        onChange={setMultimodal}
        label={t("providers.model.multimodal")}
        description={t("providers.modelForm.multimodalHint")}
      />
      <div className="field">
        <label className="field__label" htmlFor="model-effort-levels">
          {t("providers.modelForm.effortLevels.label")}
        </label>
        <input
          id="model-effort-levels"
          className="input input--mono"
          value={effortLevelsRaw}
          onChange={(event) => setEffortLevelsRaw(event.target.value)}
          placeholder={t("providers.modelForm.effortLevels.placeholder")}
          autoComplete="off"
          spellCheck={false}
        />
        <span className="field__hint">{t("providers.modelForm.effortLevels.hint")}</span>
      </div>
      {error ? (
        <div className="form-error" role="alert">
          {error}
        </div>
      ) : null}
      <div className="form-actions">
        {isEditing && onDelete ? (
          <GlassButton variant="ghost" onClick={() => void handleDelete()} disabled={submitting}>
            {t("providers.actions.delete")}
          </GlassButton>
        ) : null}
        <GlassButton variant="ghost" onClick={onCancel} disabled={submitting}>
          {t("providers.form.cancel")}
        </GlassButton>
        <GlassButton variant="primary" type="submit" disabled={submitting}>
          {submitting ? (
            <>
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
              <span>{t("providers.modelForm.saving")}</span>
            </>
          ) : (
            <span>{t("providers.form.save")}</span>
          )}
        </GlassButton>
      </div>
    </form>
  );
};
