import { useCallback, useEffect, useState, type MouseEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { GearButton } from "@/components/GearButton";
import { IconButton } from "@/components/IconButton";
import { WindowControls } from "@/components/WindowControls";
import { WindowResizeFrame } from "@/components/WindowResizeFrame";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { SessionsSidebar } from "@/features/sessions/SessionsSidebar";
import { ChatComposer } from "@/features/chat/ChatComposer";
import { ChatThread } from "@/features/chat/ChatThread";
import { ContextStrip } from "@/features/chat/ContextStrip";
import { ModelSelector } from "@/features/chat/ModelSelector";
import { useGlobalKeybindings } from "@/lib/use-global-keybindings";
import { useSettingsStore } from "@/lib/settings";
import { useComposerStore } from "@/lib/composer";
import { useSelectionStore } from "@/lib/selected-model";
import { isTauri } from "@/lib/platform";
import { useWindowBoundsSync } from "@/lib/window-bounds";
import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import type { KeybindingAction } from "@/types/settings";
import i18n from "@/i18n";

const startWindowDrag = (event: MouseEvent<HTMLElement>): void => {
  if (!isTauri() || event.button !== 0) return;
  const target = event.target;
  if (!(target instanceof Element)) return;
  if (target.closest(".app-titlebar__actions, .app-titlebar__sidebar, button, a, input, textarea")) {
    return;
  }
  event.preventDefault();
  void invoke("window_start_drag").catch((error: unknown) => {
    console.warn("window_start_drag failed", error);
  });
};

export const App = (): ReactNode => {
  const { t } = useTranslation();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const language = useSettingsStore((state) => state.language);
  const hydrate = useSettingsStore((state) => state.hydrate);
  const sidebarOpen = useSettingsStore((state) => state.sessionSidebarOpen);
  const setSidebarOpen = useSettingsStore((state) => state.setSessionSidebarOpen);
  const hydrateSelection = useSelectionStore((state) => state.hydrate);
  const clearComposer = useComposerStore((state) => state.clear);

  useEffect(() => {
    void hydrate();
  }, [hydrate]);

  useEffect(() => {
    void hydrateSelection();
  }, [hydrateSelection]);

  useEffect(() => {
    if (i18n.language !== language) {
      void i18n.changeLanguage(language);
    }
  }, [language]);

  const handleAction = useCallback(
    (action: KeybindingAction) => {
      if (action === "settings.open") setSettingsOpen(true);
      else if (action === "settings.close") setSettingsOpen(false);
      else if (action === "sidebar.toggle") setSidebarOpen(!useSettingsStore.getState().sessionSidebarOpen);
      else if (action === "chat.clear") clearComposer();
    },
    [clearComposer, setSidebarOpen],
  );

  useGlobalKeybindings(handleAction);
  useWindowBoundsSync();

  return (
    <div className="app-shell" data-sidebar-open={sidebarOpen ? "true" : "false"}>
      <WindowResizeFrame />
      <header className="app-titlebar" onMouseDown={startWindowDrag}>
        <IconButton
          label={t("sessions.toggle")}
          onClick={() => setSidebarOpen(!sidebarOpen)}
          className="app-titlebar__sidebar"
        >
          {sidebarOpen ? (
            <PanelLeftClose size={14} strokeWidth={1.5} />
          ) : (
            <PanelLeftOpen size={14} strokeWidth={1.5} />
          )}
        </IconButton>
        <span className="app-titlebar__brand" data-tauri-drag-region>
          {t("app.name")}
        </span>
        <span className="app-titlebar__drag" data-tauri-drag-region aria-hidden="true" />
        <div className="app-titlebar__actions">
          <GearButton onClick={() => setSettingsOpen(true)} />
          <WindowControls />
        </div>
      </header>
      <div className="app-body">
        <SessionsSidebar />
        <main className="app-main" aria-live="polite">
          <div className="model-bar">
            <ModelSelector />
          </div>
          <ChatThread />
          <ChatComposer />
          <ContextStrip />
        </main>
      </div>
      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
};
