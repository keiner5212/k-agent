import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { LineEditor } from "@/components/LineEditor";
import { EDITOR_SAVE_EVENT } from "@/lib/keybindings";
import {
  MAX_AGENT_PERSONALITY_LINES,
  clampPersonality,
  personalityLineCount,
  type AgentMeta,
} from "@/types/agents";

type AgentPersonalityDialogProps = {
  open: boolean;
  agent: AgentMeta | null;
  onOpenChange: (open: boolean) => void;
  onSave: (personality: string) => Promise<string | undefined>;
};

export const AgentPersonalityDialog = ({
  open,
  agent,
  onOpenChange,
  onSave,
}: AgentPersonalityDialogProps): ReactNode => {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="agents.editor.title"
      size="wide"
      surfaceStyle={{
        height: "calc(100vh - var(--titlebar-height) - var(--space-6))",
        maxHeight: "calc(100vh - var(--titlebar-height) - var(--space-6))",
      }}
    >
      {open && agent ? (
        <AgentPersonalityBody
          key={agent.path}
          agent={agent}
          onCancel={() => onOpenChange(false)}
          onSave={onSave}
        />
      ) : null}
    </Dialog>
  );
};

type AgentPersonalityBodyProps = {
  agent: AgentMeta;
  onCancel: () => void;
  onSave: (personality: string) => Promise<string | undefined>;
};

const AgentPersonalityBody = ({
  agent,
  onCancel,
  onSave,
}: AgentPersonalityBodyProps): ReactNode => {
  const { t } = useTranslation();
  const [content, setContent] = useState(() => agent.personality);
  const [original, setOriginal] = useState(() => agent.personality);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const lines = personalityLineCount(content);

  const handleSave = useCallback(async (): Promise<void> => {
    if (submitting || content === original) return;
    setSubmitting(true);
    setError(null);
    const saveError = await onSave(clampPersonality(content));
    setSubmitting(false);
    if (saveError) {
      setError(saveError);
      return;
    }
    setOriginal(content);
  }, [submitting, content, original, onSave]);

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
      <div className="skill-editor__path" title={agent.path}>
        {agent.path}
      </div>
      {error ? (
        <div className="form-error" role="alert">
          {error}
        </div>
      ) : null}
      <LineEditor
        value={content}
        onChange={(next) => setContent(clampPersonality(next))}
        maxLines={MAX_AGENT_PERSONALITY_LINES}
      />
      <div className="skill-editor__meta">
        <span className="agent-form__count">
          {t("agents.form.personalityCount", {
            count: lines,
            max: MAX_AGENT_PERSONALITY_LINES,
          })}
        </span>
        <span className="skill-editor__dirty" data-dirty={dirty ? "true" : "false"}>
          {dirty ? t("agents.editor.dirty") : t("agents.editor.clean")}
        </span>
      </div>
      <div className="form-actions">
        <GlassButton variant="ghost" onClick={handleRevert} disabled={submitting || !dirty}>
          {t("agents.editor.revert")}
        </GlassButton>
        <GlassButton
          variant="ghost"
          onClick={() => void handleSave()}
          disabled={submitting || !dirty}
        >
          {submitting ? (
            <>
              <Loader2 size={14} strokeWidth={1.5} className="spin" />
              <span>{t("agents.editor.saving")}</span>
            </>
          ) : (
            <span>{t("agents.editor.save")}</span>
          )}
        </GlassButton>
        <GlassButton variant="primary" onClick={onCancel} disabled={submitting}>
          {t("agents.editor.done")}
        </GlassButton>
      </div>
    </div>
  );
};
