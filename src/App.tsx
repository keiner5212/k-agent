import { useCallback, useEffect, useState, type MouseEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { GearButton } from "@/components/GearButton";
import { WindowControls } from "@/components/WindowControls";
import { WindowResizeFrame } from "@/components/WindowResizeFrame";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { useGlobalKeybindings } from "@/lib/use-global-keybindings";
import { useSettingsStore } from "@/lib/settings";
import { isTauri } from "@/lib/platform";
import { useWindowBoundsSync } from "@/lib/window-bounds";
import type { KeybindingAction } from "@/types/settings";
import i18n from "@/i18n";

const startWindowDrag = (event: MouseEvent<HTMLElement>): void => {
  if (!isTauri() || event.button !== 0) return;
  const target = event.target;
  if (!(target instanceof Element)) return;
  if (target.closest(".app-titlebar__actions")) return;
  event.preventDefault();
  void invoke("window_start_drag").catch((error: unknown) => {
    console.warn("window_start_drag failed", error);
  });
};

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

  const handleAction = useCallback((action: KeybindingAction) => {
    if (action === "settings.open") setSettingsOpen(true);
    else if (action === "settings.close") setSettingsOpen(false);
  }, []);

  useGlobalKeybindings(handleAction);
  useWindowBoundsSync();

  const settingsShortcut = useSettingsStore((state) => state.keybindings["settings.open"]);

  return (
    <div className="app-shell">
      <WindowResizeFrame />
      <header className="app-titlebar" onMouseDown={startWindowDrag}>
        <span className="app-titlebar__brand" data-tauri-drag-region>
          {t("app.name")}
        </span>
        <span className="app-titlebar__drag" data-tauri-drag-region aria-hidden="true" />
        <div className="app-titlebar__actions">
          <GearButton onClick={() => setSettingsOpen(true)} />
          <WindowControls />
        </div>
      </header>
      <main className="app-main" aria-live="polite">
        {hydrated ? (
          <span className="app-main__hint">
            {t("app.shortcut_hint", { shortcut: settingsShortcut })}
          </span>
        ) : null}
      </main>
      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
};
