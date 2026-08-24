import { useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Bug, Keyboard, Plug, Sliders } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { ProvidersPanel } from "@/features/providers/ProvidersPanel";
import { SETTINGS_REGISTRY } from "./registry-data";
import { SettingItem } from "./SettingItem";
import { KeybindingField } from "./KeybindingField";
import type { SettingsSectionDef } from "./registry";
import type { KeybindingAction } from "@/types/settings";

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
  { id: "debug", icon: Bug, labelKey: "settings.sections.debug" },
];

type SettingsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

const itemMatches = (item: SettingsSectionDef["items"][number], query: string, t: (key: string) => string): boolean => {
  const text = [t(item.titleKey), t(item.descriptionKey), ...(item.keywords ?? [])]
    .join(" ")
    .toLowerCase();
  return text.includes(query);
};

const sectionMatchesQuery = (
  section: SettingsSectionDef,
  query: string,
  t: (key: string) => string,
): boolean => {
  const text = [
    t(section.titleKey),
    section.descriptionKey ? t(section.descriptionKey) : "",
    ...(section.keywords ?? []),
  ]
    .join(" ")
    .toLowerCase();
  return text.includes(query);
};

export const SettingsDialog = ({ open, onOpenChange }: SettingsDialogProps): ReactNode => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [query, setQuery] = useState("");

  const trimmed = query.trim().toLowerCase();

  const sections = useMemo(() => {
    if (trimmed.length === 0) return SETTINGS_REGISTRY;
    return SETTINGS_REGISTRY.flatMap((section) => {
      if (sectionMatchesQuery(section, trimmed, t)) return [section];
      const items = section.items.filter((item) => itemMatches(item, trimmed, t));
      return items.length > 0 ? [{ ...section, items }] : [];
    });
  }, [trimmed, t]);

  const visibleTabs = TABS.filter((tab) => sections.some((section) => section.id === tab.id));
  const resolvedTab: SettingsTab = visibleTabs.some((tab) => tab.id === activeTab)
    ? activeTab
    : (visibleTabs[0]?.id ?? "general");

  const activeSection = sections.find((section) => section.id === resolvedTab);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="settings.title"
      footer={
        <GlassButton variant="primary" onClick={() => onOpenChange(false)}>
          {t("settings.close")}
        </GlassButton>
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
          {visibleTabs.length > 0 ? (
            <ul className="settings-sidebar__list" role="tablist">
              {visibleTabs.map((tab) => {
                const Icon = tab.icon;
                const isActive = tab.id === resolvedTab;
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
          ) : null}
        </nav>
        <div className="settings-content" role="tabpanel">
          <div key={resolvedTab} className="settings-pane">
          {visibleTabs.length === 0 ? (
            <div className="settings-empty">{t("settings.searchEmpty")}</div>
          ) : resolvedTab === "providers" ? (
            <ProvidersPanel />
          ) : resolvedTab === "keybindings" ? (
            <KeybindingsPanel items={activeSection?.items ?? []} query={trimmed} />
          ) : (
            <div className="settings-list">
              {(activeSection?.items ?? []).map((item) => (
                <SettingItem key={item.id} item={item} query={query.trim()} />
              ))}
            </div>
          )}
          </div>
        </div>
      </div>
    </Dialog>
  );
};

const KeybindingsPanel = ({
  items,
  query,
}: {
  items: SettingsSectionDef["items"];
  query: string;
}): ReactNode => {
  const { t } = useTranslation();

  return (
    <section className="kbd-section">
      <header className="kbd-section__head">
        <h3 className="section__heading">{t("settings.sections.keybindings")}</h3>
        <p className="section__description">{t("settings.keybindings.description")}</p>
      </header>
      {items.length === 0 ? (
        <div className="settings-empty">{t("settings.searchEmpty")}</div>
      ) : (
        <table className="kbd-table">
          <thead>
            <tr>
              <th>{t("settings.keybindings.action")}</th>
              <th>{t("settings.keybindings.shortcut")}</th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.id}>
                <td>{highlightLabel(t(item.titleKey), query)}</td>
                <td>
                  <KeybindingField action={item.id as KeybindingAction} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
};

const highlightLabel = (text: string, query: string): ReactNode => {
  if (query.length === 0) return text;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx === -1) return text;
  return (
    <>
      {text.slice(0, idx)}
      <mark className="search-mark">{text.slice(idx, idx + query.length)}</mark>
      {text.slice(idx + query.length)}
    </>
  );
};
