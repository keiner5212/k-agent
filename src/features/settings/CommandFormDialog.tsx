import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { MAX_PROMPT_TEMPLATE, type PromptCommand } from "@/types/settings";

type CommandFormDialogProps = {
  open: boolean;
  mode: "create" | "edit";
  initial: PromptCommand | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (command: PromptCommand) => string | undefined;
};

export const CommandFormDialog = ({
  open,
  mode,
  initial,
  onOpenChange,
  onSubmit,
}: CommandFormDialogProps): ReactNode => {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? "");
  const [description, setDescription] = useState(initial?.description ?? "");
  const [template, setTemplate] = useState(initial?.template ?? "");
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault();
    const saveError = onSubmit({
      name: name.trim(),
      description: description.trim(),
      template: template.trim(),
    });
    if (saveError) {
      setError(saveError);
      return;
    }
    onOpenChange(false);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey={
        mode === "create"
          ? "settings.commands.form.createTitle"
          : "settings.commands.form.editTitle"
      }
      size="narrow"
      placement="center"
    >
      <form className="command-form" onSubmit={handleSubmit}>
        <div className="field">
          <label className="field__label" htmlFor="command-name">
            {t("settings.commands.name")}
          </label>
          <input
            id="command-name"
            className="input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            required
          />
        </div>
        <div className="field">
          <label className="field__label" htmlFor="command-description">
            {t("settings.commands.descriptionLabel")}
          </label>
          <input
            id="command-description"
            className="input"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            autoComplete="off"
          />
        </div>
        <div className="field">
          <label className="field__label" htmlFor="command-template">
            {t("settings.commands.template")}
          </label>
          <textarea
            id="command-template"
            className="input command-form__textarea"
            value={template}
            onChange={(event) => setTemplate(event.target.value)}
            maxLength={MAX_PROMPT_TEMPLATE}
            required
          />
        </div>
        {error ? (
          <div className="form-error" role="alert">
            {error}
          </div>
        ) : null}
        <div className="form-actions">
          <GlassButton variant="secondary" onClick={() => onOpenChange(false)}>
            {t("settings.commands.form.cancel")}
          </GlassButton>
          <GlassButton variant="primary" type="submit" disabled={!name.trim() || !template.trim()}>
            {t("settings.commands.form.save")}
          </GlassButton>
        </div>
      </form>
    </Dialog>
  );
};
