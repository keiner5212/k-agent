import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { LineEditor } from "@/components/LineEditor";
import { EDITOR_SAVE_EVENT } from "@/lib/keybindings";
import { useSkillsStore } from "@/lib/skills";

type SkillEditorDialogProps = {
  open: boolean;
  skillPath: string | null;
  onOpenChange: (open: boolean) => void;
  onSave: (content: string) => Promise<string | undefined>;
};

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
    void useSkillsStore
      .getState()
      .readFile(skillPath)
      .then((result) => {
        if (cancelled) return;
        if (result.error) {
          setError(result.error);
          return;
        }
        const value = result.content ?? "";
        setContent(value);
        setOriginal(value);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [skillPath]);

  const handleSave = useCallback(async (): Promise<boolean> => {
    if (submitting || loading) return false;
    if (content === original) return true;
    setSubmitting(true);
    setError(null);
    const saveError = await onSave(content);
    setSubmitting(false);
    if (saveError) {
      setError(saveError);
      return false;
    }
    setOriginal(content);
    return true;
  }, [submitting, loading, content, original, onSave]);

  const handleDone = async (): Promise<void> => {
    const saved = await handleSave();
    if (saved) onCancel();
  };

  const handleRevert = (): void => {
    setContent(original);
  };

  const dirty = content !== original;

  useEffect(() => {
    const onSave = (): void => {
      void handleSave();
    };
    window.addEventListener(EDITOR_SAVE_EVENT, onSave);
    return () => window.removeEventListener(EDITOR_SAVE_EVENT, onSave);
  }, [handleSave]);

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
        <GlassButton
          variant="ghost"
          onClick={handleRevert}
          disabled={submitting || !dirty || loading}
        >
          {t("skills.editor.revert")}
        </GlassButton>
        <GlassButton
          variant="ghost"
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
        <GlassButton
          variant="primary"
          onClick={() => void handleDone()}
          disabled={submitting || loading}
        >
          {t("skills.editor.done")}
        </GlassButton>
      </div>
    </div>
  );
};
