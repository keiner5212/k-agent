import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { LineEditor } from "@/components/LineEditor";
import { EDITOR_SAVE_EVENT } from "@/lib/keybindings";
import { runEstimateTokensJob } from "@/lib/jobs";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { formatContextWindow } from "@/types/providers";
import type { AgentsMdFile } from "@/types/agents-md";

type AgentsMdEditorDialogProps = {
  open: boolean;
  file: AgentsMdFile | null;
  onOpenChange: (open: boolean) => void;
  onSave: (content: string) => Promise<string | undefined>;
};

export const AgentsMdEditorDialog = ({
  open,
  file,
  onOpenChange,
  onSave,
}: AgentsMdEditorDialogProps): ReactNode => {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey={file?.exists ? "agentsMd.editor.editTitle" : "agentsMd.editor.createTitle"}
      size="wide"
      surfaceStyle={{
        height: "calc(100vh - var(--titlebar-height) - var(--space-6))",
        maxHeight: "calc(100vh - var(--titlebar-height) - var(--space-6))",
      }}
    >
      {open && file ? (
        <AgentsMdEditorBody
          key={`${file.kind}:${file.path}`}
          file={file}
          onCancel={() => onOpenChange(false)}
          onSave={onSave}
        />
      ) : null}
    </Dialog>
  );
};

type AgentsMdEditorBodyProps = {
  file: AgentsMdFile;
  onCancel: () => void;
  onSave: (content: string) => Promise<string | undefined>;
};

const AgentsMdEditorBody = ({ file, onCancel, onSave }: AgentsMdEditorBodyProps): ReactNode => {
  const { t } = useTranslation();
  const initial = file.exists ? file.content : "";
  const [content, setContent] = useState(initial);
  const [original, setOriginal] = useState(initial);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tokens, setTokens] = useState(() => estimateTokensFromText(initial));

  useEffect(() => {
    let alive = true;
    const timer = window.setTimeout(() => {
      void runEstimateTokensJob(content).then((value) => {
        if (alive) setTokens(value);
      });
    }, 120);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [content]);

  const handleSave = useCallback(async (): Promise<void> => {
    if (submitting) return;
    if (file.exists && content === original) return;
    setSubmitting(true);
    setError(null);
    const saveError = await onSave(content);
    setSubmitting(false);
    if (saveError) {
      setError(saveError);
      return;
    }
    setOriginal(content);
  }, [submitting, file.exists, content, original, onSave]);

  const handleRevert = (): void => {
    setContent(original);
  };

  const dirty = content !== original;

  useEffect(() => {
    const onKeySave = (): void => {
      void handleSave();
    };
    window.addEventListener(EDITOR_SAVE_EVENT, onKeySave);
    return () => window.removeEventListener(EDITOR_SAVE_EVENT, onKeySave);
  }, [handleSave]);

  return (
    <div className="skill-editor">
      <div className="skill-editor__path" title={file.path}>
        {file.path}
      </div>
      {error ? (
        <div className="form-error" role="alert">
          {error}
        </div>
      ) : null}
      <LineEditor value={content} onChange={setContent} />
      <div className="skill-editor__meta">
        <span className="agent-form__count">
          {t("agentsMd.tokens", { value: formatContextWindow(tokens) })}
        </span>
        <span className="skill-editor__dirty" data-dirty={dirty ? "true" : "false"}>
          {dirty ? t("agentsMd.editor.dirty") : t("agentsMd.editor.clean")}
        </span>
      </div>
      <div className="form-actions">
        <GlassButton variant="ghost" onClick={handleRevert} disabled={submitting || !dirty}>
          {t("agentsMd.editor.revert")}
        </GlassButton>
        <GlassButton
          variant="ghost"
          onClick={() => void handleSave()}
          disabled={submitting || (file.exists && !dirty)}
        >
          {submitting ? (
            <>
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
              <span>{t("agentsMd.editor.saving")}</span>
            </>
          ) : (
            <span>{t("agentsMd.editor.save")}</span>
          )}
        </GlassButton>
        <GlassButton variant="primary" onClick={onCancel} disabled={submitting}>
          {t("agentsMd.editor.done")}
        </GlassButton>
      </div>
    </div>
  );
};
