import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard, Plug, Sliders } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GeneralSection } from "./GeneralSection";
import { KeybindingSection } from "./KeybindingSection";
import { ProvidersPanel } from "@/features/providers/ProvidersPanel";

type SettingsTab = "general" | "providers" | "keybindings";

type TabMeta = {
  id: SettingsTab;
  icon: typeof Sliders;
  labelKey: string;
};

const TABS: readonly TabMeta[] = [
  { id: "general", icon: Sliders, labelKey: "settings.sections.general" },
  { id: "providers", icon: Plug, labelKey: "settings.sections.providers" },
  { id: "keybindings", icon: Keyboard, labelKey: "settings.sections.keybindings" },
];

const renderTab = (tab: SettingsTab): ReactNode => {
  switch (tab) {
    case "general":
      return <GeneralSection />;
    case "providers":
      return <ProvidersPanel />;
    case "keybindings":
      return <KeybindingSection />;
  }
};

type SettingsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export const SettingsDialog = ({ open, onOpenChange }: SettingsDialogProps): ReactNode => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");

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
      <div className="settings-layout">
        <nav className="settings-sidebar" aria-label={t("settings.title")}>
          <div className="settings-sidebar__search">
            <input
              type="search"
              className="input settings-sidebar__search-input"
              placeholder={t("settings.searchPlaceholder")}
              aria-label={t("settings.searchPlaceholder")}
            />
          </div>
          <ul className="settings-sidebar__list" role="tablist">
            {TABS.map((tab) => {
              const Icon = tab.icon;
              const isActive = tab.id === activeTab;
              return (
                <li key={tab.id}>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={isActive}
                    className="settings-sidebar__item"
                    data-active={isActive}
                    onClick={() => setActiveTab(tab.id)}
                  >
                    <Icon size={14} strokeWidth={1.5} className="settings-sidebar__icon" />
                    <span>{t(tab.labelKey)}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </nav>
        <div className="settings-content" role="tabpanel">
          {renderTab(activeTab)}
        </div>
      </div>
    </Dialog>
  );
};
