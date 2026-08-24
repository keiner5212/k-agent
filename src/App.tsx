import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { GearButton } from "@/components/GearButton";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { useGlobalKeybindings } from "@/lib/use-global-keybindings";
import { useSettingsStore } from "@/lib/settings";
import i18n from "@/i18n";

export const App = (): ReactNode => {
  const { t } = useTranslation();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const language = useSettingsStore((state) => state.language);
  const hydrated = useSettingsStore((state) => state.hydrated);
  const hydrate = useSettingsStore((state) => state.hydrate);

  useEffect(() => {
    void hydrate();
  }, [hydrate]);

  useEffect(() => {
    if (i18n.language !== language) {
      void i18n.changeLanguage(language);
    }
  }, [language]);

  const handleAction = useCallback((action: string) => {
    if (action === "settings.open") setSettingsOpen(true);
    else if (action === "settings.close") setSettingsOpen(false);
  }, []);

  useGlobalKeybindings(handleAction);

  const settingsShortcut = useSettingsStore((state) => state.keybindings["settings.open"]);

  return (
    <div className="app-shell">
      <header className="app-titlebar">
        <span className="app-titlebar__brand">{t("app.name")}</span>
        <GearButton onClick={() => setSettingsOpen(true)} />
      </header>
      <main className="app-main" aria-live="polite">
        {hydrated ? <span>{t("app.shortcut_hint", { shortcut: settingsShortcut })}</span> : null}
      </main>
      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
};
