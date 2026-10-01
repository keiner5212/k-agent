import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type DeleteCommandDialogProps = {
  open: boolean;
  name: string;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
};

export const DeleteCommandDialog = ({
  open,
  name,
  onOpenChange,
  onConfirm,
}: DeleteCommandDialogProps): ReactNode => {
  const { t } = useTranslation();

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="settings.commands.delete.title"
      placement="center"
      size="narrow"
    >
      <div className="command-delete">
        <p className="command-delete__body">{t("settings.commands.delete.body", { name })}</p>
        <div className="form-actions">
          <GlassButton variant="secondary" onClick={() => onOpenChange(false)}>
            {t("settings.commands.delete.cancel")}
          </GlassButton>
          <GlassButton
            variant="danger"
            onClick={() => {
              onConfirm();
              onOpenChange(false);
            }}
          >
            {t("settings.commands.delete.confirm")}
          </GlassButton>
        </div>
      </div>
    </Dialog>
  );
};
