import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type DeleteAgentsMdDialogProps = {
  open: boolean;
  path: string;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<string | undefined>;
};

export const DeleteAgentsMdDialog = ({
  open,
  path,
  onOpenChange,
  onConfirm,
}: DeleteAgentsMdDialogProps): ReactNode => {
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
      titleKey="agentsMd.delete.title"
      placement="center"
      size="narrow"
    >
      <div className="delete-skill">
        <p className="delete-skill__body">{t("agentsMd.delete.body", { path })}</p>
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
            {t("agentsMd.delete.cancel")}
          </GlassButton>
          <GlassButton variant="danger" onClick={() => void handleConfirm()} disabled={submitting}>
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>{t("agentsMd.delete.deleting")}</span>
              </>
            ) : (
              <span>{t("agentsMd.delete.confirm")}</span>
            )}
          </GlassButton>
        </div>
      </div>
    </Dialog>
  );
};
