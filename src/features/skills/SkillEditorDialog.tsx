import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { LineEditor } from "@/components/LineEditor";

type SkillEditorDialogProps = {
  open: boolean;
  skillPath: string | null;
  onOpenChange: (open: boolean) => void;
  onSave: (content: string) => Promise<string | undefined>;
};

const errorMessage = (error: unknown): string =>
  error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "Failed to read skill";

export const SkillEditorDialog = ({
  open,
  skillPath,
  onOpenChange,
  onSave,
}: SkillEditorDialogProps): ReactNode => {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="skills.editor.title"
      size="wide"
      surfaceStyle={{
        height: "calc(100vh - var(--titlebar-height) - var(--space-6))",
        maxHeight: "calc(100vh - var(--titlebar-height) - var(--space-6))",
      }}
    >
      {open && skillPath ? (
        <SkillEditorBody
          key={skillPath}
          skillPath={skillPath}
          onCancel={() => onOpenChange(false)}
          onSave={onSave}
        />
      ) : null}
    </Dialog>
  );
};

type SkillEditorBodyProps = {
  skillPath: string;
  onCancel: () => void;
  onSave: (content: string) => Promise<string | undefined>;
};

const SkillEditorBody = ({ skillPath, onCancel, onSave }: SkillEditorBodyProps): ReactNode => {
  const { t } = useTranslation();
  const [content, setContent] = useState("");
  const [original, setOriginal] = useState("");
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<string>("read_skill_file", { input: { path: skillPath } })
      .then((value) => {
        if (cancelled) return;
        setContent(value);
        setOriginal(value);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(errorMessage(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [skillPath]);

  const handleSave = async (): Promise<void> => {
    setSubmitting(true);
    setError(null);
    const saveError = await onSave(content);
    setSubmitting(false);
    if (saveError) {
      setError(saveError);
      return;
    }
    setOriginal(content);
    onCancel();
  };

  const handleRevert = (): void => {
    setContent(original);
  };

  const dirty = content !== original;

  return (
    <div className="skill-editor">
      <div className="skill-editor__path" title={skillPath}>
        {skillPath}
      </div>
      {error ? (
        <div className="form-error" role="alert">
          {error}
        </div>
      ) : null}
      {loading ? (
        <div className="skill-editor__loading">
          <Loader2 size={14} strokeWidth={1.5} className="spin" />
          <span>{t("skills.editor.loading")}</span>
        </div>
      ) : (
        <LineEditor value={content} onChange={setContent} />
      )}
      <div className="skill-editor__meta">
        <span className="skill-editor__dirty" data-dirty={dirty ? "true" : "false"}>
          {dirty ? t("skills.editor.dirty") : t("skills.editor.clean")}
        </span>
      </div>
      <div className="form-actions">
        <GlassButton variant="ghost" onClick={onCancel} disabled={submitting}>
          {t("skills.editor.cancel")}
        </GlassButton>
        <GlassButton
          variant="ghost"
          onClick={handleRevert}
          disabled={submitting || !dirty || loading}
        >
          {t("skills.editor.revert")}
        </GlassButton>
        <GlassButton
          variant="primary"
          onClick={() => void handleSave()}
          disabled={submitting || loading || !dirty}
        >
          {submitting ? (
            <>
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
              <span>{t("skills.editor.saving")}</span>
            </>
          ) : (
            <span>{t("skills.editor.save")}</span>
          )}
        </GlassButton>
      </div>
    </div>
  );
};
