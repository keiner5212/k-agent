import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Terminal } from "lucide-react";
import { useSettingsStore } from "@/lib/settings";

export const ComposerShellChip = (): ReactNode => {
  const { t } = useTranslation();
  const animationsEnabled = useSettingsStore((state) => state.animationsEnabled);

  return (
    <div
      key={animationsEnabled ? "shell" : "mode"}
      className={`composer-shell-chip${animationsEnabled ? " composer-shell-chip--enter" : ""}`}
      title={t("chat.composer.mode.hint")}
    >
      <Terminal size={12} strokeWidth={1.75} className="composer-shell-chip__icon" />
      <span className="composer-shell-chip__label">{t("chat.composer.mode.shell")}</span>
    </div>
  );
};
