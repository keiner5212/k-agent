import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type DeleteMcpServerDialogProps = {
  open: boolean;
  serverName: string;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<string | undefined>;
};

export const DeleteMcpServerDialog = ({
  open,
  serverName,
  onOpenChange,
  onConfirm,
}: DeleteMcpServerDialogProps): ReactNode => {
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
      titleKey="mcpServers.delete.title"
      placement="center"
      size="narrow"
    >
      <div className="delete-skill">
        <p className="delete-skill__body">{t("mcpServers.delete.body", { name: serverName })}</p>
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
            {t("mcpServers.delete.cancel")}
          </GlassButton>
          <GlassButton variant="danger" onClick={() => void handleConfirm()} disabled={submitting}>
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>{t("mcpServers.delete.deleting")}</span>
              </>
            ) : (
              <span>{t("mcpServers.delete.confirm")}</span>
            )}
          </GlassButton>
        </div>
      </div>
    </Dialog>
  );
};
