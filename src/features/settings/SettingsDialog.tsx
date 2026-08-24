import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { GeneralSection } from "./GeneralSection";
import { KeybindingSection } from "./KeybindingSection";

type SettingsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export const SettingsDialog = ({ open, onOpenChange }: SettingsDialogProps): ReactNode => {
  const { t } = useTranslation();

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="settings.title"
      footer={
        <button type="button" className="btn btn--primary" onClick={() => onOpenChange(false)}>
          {t("settings.close")}
        </button>
      }
    >
      <GeneralSection />
      <KeybindingSection />
    </Dialog>
  );
};
