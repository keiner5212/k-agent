import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type DeleteAgentDialogProps = {
  open: boolean;
  agentName: string;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<string | undefined>;
};

export const DeleteAgentDialog = ({
  open,
  agentName,
  onOpenChange,
  onConfirm,
}: DeleteAgentDialogProps): ReactNode => {
  const { t } = useTranslation();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleConfirm = async (): Promise<void> => {
    setSubmitting(true);
    setError(null);
    const deleteError = await onConfirm();
    setSubmitting(false);
    if (deleteError) {
      setError(deleteError);
      return;
    }
    onOpenChange(false);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="agents.delete.title"
      placement="center"
      size="narrow"
    >
      <div className="delete-skill">
        <p className="delete-skill__body">{t("agents.delete.body", { name: agentName })}</p>
        {error ? (
          <div className="form-error" role="alert">
            {error}
          </div>
        ) : null}
        <div className="form-actions">
          <GlassButton variant="ghost" onClick={() => onOpenChange(false)} disabled={submitting}>
            {t("agents.delete.cancel")}
          </GlassButton>
          <GlassButton variant="primary" onClick={() => void handleConfirm()} disabled={submitting}>
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>{t("agents.delete.deleting")}</span>
              </>
            ) : (
              <span>{t("agents.delete.confirm")}</span>
            )}
          </GlassButton>
        </div>
      </div>
    </Dialog>
  );
};
