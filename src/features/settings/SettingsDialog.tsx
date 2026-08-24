import { useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Keyboard, Plug, Sliders } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { ProvidersPanel } from "@/features/providers/ProvidersPanel";
import { useProvidersStore } from "@/lib/providers";
import { useEffect } from "react";
import { SETTINGS_REGISTRY } from "./registry-data";
import { SettingItem } from "./SettingItem";
import type { SettingsSectionDef } from "./registry";

type SettingsTab = SettingsSectionDef["id"];

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

const renderPanel = (tab: SettingsTab): ReactNode => {
  if (tab === "providers") return <ProvidersPanel />;
  return null;
};

type SettingsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export const SettingsDialog = ({ open, onOpenChange }: SettingsDialogProps): ReactNode => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [query, setQuery] = useState("");
  const loadProviders = useProvidersStore((state) => state.load);

  useEffect(() => {
    if (open && activeTab === "providers") {
      void loadProviders();
    }
  }, [open, activeTab, loadProviders]);

  const sections = useMemo(() => {
    const trimmed = query.trim();
    if (trimmed.length === 0) return SETTINGS_REGISTRY;
    return SETTINGS_REGISTRY.map((section) => ({
      ...section,
      items: section.items.filter((item) => {
        const text = [t(item.titleKey), t(item.descriptionKey), ...(item.keywords ?? [])]
          .join(" ")
          .toLowerCase();
        return text.includes(trimmed.toLowerCase());
      }),
    })).filter((section) => section.items.length > 0 || section.id === "providers");
  }, [query, t]);

  const visibleSectionIds = new Set(sections.map((s) => s.id));

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
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          <ul className="settings-sidebar__list" role="tablist">
            {TABS.map((tab) => {
              const Icon = tab.icon;
              const isActive = tab.id === activeTab;
              const hasMatches = visibleSectionIds.has(tab.id);
              return (
                <li key={tab.id}>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={isActive}
                    className="settings-sidebar__item"
                    data-active={isActive}
                    data-empty={!hasMatches && query.length > 0}
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
          {activeTab === "providers" ? (
            renderPanel("providers")
          ) : (
            <div className="settings-list">
              {sections
                .filter((s) => s.id === activeTab)
                .flatMap((section) =>
                  section.items.map((item) => (
                    <SettingItem key={item.id} item={item} query={query.trim()} />
                  )),
                )}
              {sections.find((s) => s.id === activeTab)?.items.length === 0 ? (
                <div className="settings-empty">{t("settings.searchEmpty")}</div>
              ) : null}
            </div>
          )}
        </div>
      </div>
    </Dialog>
  );
};
