import type { ReactNode } from "react";
import { Settings } from "lucide-react";
import { useTranslation } from "react-i18next";

type GearButtonProps = {
  onClick: () => void;
  shortcutLabel?: string;
};

export const GearButton = ({ onClick, shortcutLabel }: GearButtonProps): ReactNode => {
  const { t } = useTranslation();
  const label = shortcutLabel ?? t("settings.title");

  return (
    <button
      type="button"
      className="icon-button"
      onClick={onClick}
      aria-label={label}
      title={label}
    >
      <Settings size={16} strokeWidth={1.5} />
    </button>
  );
};
