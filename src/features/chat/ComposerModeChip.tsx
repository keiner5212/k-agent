import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Terminal } from "lucide-react";
import { useSettingsStore } from "@/lib/settings";

export const ComposerShellChip = (): ReactNode => {
  const { t } = useTranslation();
  const animationsEnabled = useSettingsStore((state) => state.animationsEnabled);
  const windowAnimationsEnabled = useSettingsStore((state) => state.windowAnimationsEnabled);
  const playEnter = animationsEnabled && windowAnimationsEnabled;

  return (
    <div
      key={playEnter ? "shell" : "mode"}
      className={`composer-shell-chip${playEnter ? " composer-shell-chip--enter" : ""}`}
      title={t("chat.composer.mode.hint")}
    >
      <Terminal size={12} strokeWidth={1.75} className="composer-shell-chip__icon" />
      <span className="composer-shell-chip__label">{t("chat.composer.mode.shell")}</span>
    </div>
  );
};
