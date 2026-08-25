import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type DeleteProviderDialogProps = {
  open: boolean;
  providerName: string;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => Promise<string | undefined>;
};

export const DeleteProviderDialog = ({
  open,
  providerName,
  onOpenChange,
  onConfirm,
}: DeleteProviderDialogProps): ReactNode => {
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
      titleKey="providers.delete.title"
      placement="center"
      size="narrow"
    >
      <div className="delete-skill">
        <p className="delete-skill__body">{t("providers.delete.body", { name: providerName })}</p>
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
            {t("providers.delete.cancel")}
          </GlassButton>
          <GlassButton variant="danger" onClick={() => void handleConfirm()} disabled={submitting}>
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>{t("providers.delete.deleting")}</span>
              </>
            ) : (
              <span>{t("providers.delete.confirm")}</span>
            )}
          </GlassButton>
        </div>
      </div>
    </Dialog>
  );
};
