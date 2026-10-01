import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type CreateSkillDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (name: string, description: string) => Promise<string | undefined>;
};

export const CreateSkillDialog = ({
  open,
  onOpenChange,
  onSubmit,
}: CreateSkillDialogProps): ReactNode => {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>): Promise<void> => {
    event.preventDefault();
    if (!name.trim()) {
      setError(t("skills.form.errors.nameRequired"));
      return;
    }
    setSubmitting(true);
    setError(null);
    const saveError = await onSubmit(name.trim(), description.trim());
    setSubmitting(false);
    if (saveError) {
      setError(saveError);
      return;
    }
    setName("");
    setDescription("");
    onOpenChange(false);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="skills.form.createTitle"
      size="narrow"
      placement="center"
    >
      <form className="skill-form" onSubmit={(event) => void handleSubmit(event)}>
        <div className="field">
          <label className="field__label" htmlFor="skill-name">
            {t("skills.form.name")}
          </label>
          <input
            id="skill-name"
            className="input input--mono"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t("skills.form.namePlaceholder")}
            autoComplete="off"
            spellCheck={false}
            pattern="[a-z0-9][a-z0-9_\-]*"
            title={t("skills.form.nameHint")}
            required
          />
          <span className="field__hint">{t("skills.form.nameHint")}</span>
        </div>
        <div className="field">
          <label className="field__label" htmlFor="skill-description">
            {t("skills.form.description")}
          </label>
          <textarea
            id="skill-description"
            className="input skill-form__textarea"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder={t("skills.form.descriptionPlaceholder")}
            rows={4}
            autoComplete="off"
          />
          <span className="field__hint">{t("skills.form.descriptionHint")}</span>
        </div>
        {error ? (
          <div className="form-error" role="alert">
            {error}
          </div>
        ) : null}
        <div className="form-actions">
          <GlassButton
            variant="secondary"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            {t("skills.form.cancel")}
          </GlassButton>
          <GlassButton variant="primary" type="submit" disabled={submitting || !name.trim()}>
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>{t("skills.form.saving")}</span>
              </>
            ) : (
              <span>{t("skills.form.save")}</span>
            )}
          </GlassButton>
        </div>
      </form>
    </Dialog>
  );
};
