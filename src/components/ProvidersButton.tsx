import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Plug } from "lucide-react";

type ProvidersButtonProps = {
  onClick: () => void;
};

export const ProvidersButton = ({ onClick }: ProvidersButtonProps): ReactNode => {
  const { t } = useTranslation();
  const label = t("providers.title");

  return (
    <button
      type="button"
      className="icon-button"
      onClick={onClick}
      aria-label={label}
      title={label}
    >
      <Plug size={16} strokeWidth={1.5} />
    </button>
  );
};
