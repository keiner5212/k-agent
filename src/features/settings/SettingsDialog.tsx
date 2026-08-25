import { useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  Brain,
  Bug,
  Bot,
  Code2,
  Keyboard,
  MessageSquare,
  Plug,
  ScrollText,
  Sliders,
  Sparkles,
} from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { Table, type TableColumn } from "@/components/Table";
import { highlightMatch } from "@/lib/highlight";
import { ProvidersPanel } from "@/features/providers/ProvidersPanel";
import { SkillsPanel } from "@/features/skills/SkillsPanel";
import { AgentsPanel } from "@/features/agents/AgentsPanel";
import { AgentsMdPanel } from "@/features/agents-md/AgentsMdPanel";
import { LspsPanel } from "@/features/lsps/LspsPanel";
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

const SECTION_ICONS: Record<SettingsSectionDef["id"], typeof Sliders> = {
  general: Sliders,
  chat: MessageSquare,
  modelChoices: Brain,
  lsps: Code2,
  providers: Plug,
  skills: Sparkles,
  agents: Bot,
  agentsMd: ScrollText,
  keybindings: Keyboard,
  debug: Bug,
};

const TABS: readonly TabMeta[] = SETTINGS_REGISTRY.map((section) => ({
  id: section.id,
  icon: SECTION_ICONS[section.id],
  labelKey: section.titleKey,
}));

type SettingsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

const itemMatches = (
  item: SettingsSectionDef["items"][number],
  query: string,
  t: (key: string) => string,
): boolean => {
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
  const densePane =
    resolvedTab === "skills" ||
    resolvedTab === "agents" ||
    resolvedTab === "agentsMd" ||
    resolvedTab === "lsps";

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="settings.title"
      size="wide"
      surfaceStyle={{
        height: "calc(100vh - var(--titlebar-height) - var(--space-6))",
        maxHeight: "calc(100vh - var(--titlebar-height) - var(--space-6))",
      }}
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
          <div
            key={resolvedTab}
            className={densePane ? "settings-pane settings-pane--dense" : "settings-pane"}
          >
            {visibleTabs.length === 0 ? (
              <div className="settings-empty">{t("settings.searchEmpty")}</div>
            ) : resolvedTab === "providers" ? (
              <ProvidersPanel query={query.trim()} />
            ) : resolvedTab === "skills" ? (
              <SkillsPanel query={query.trim()} />
            ) : resolvedTab === "agents" ? (
              <AgentsPanel query={query.trim()} />
            ) : resolvedTab === "agentsMd" ? (
              <AgentsMdPanel query={query.trim()} />
            ) : resolvedTab === "lsps" ? (
              <LspsPanel items={activeSection?.items ?? []} query={query.trim()} />
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
  const columns = useMemo(
    (): TableColumn<SettingsSectionDef["items"][number]>[] => [
      {
        id: "action",
        header: t("settings.keybindings.action"),
        className: "data-table__desc",
        render: (item) => highlightMatch(t(item.titleKey), query),
      },
      {
        id: "shortcut",
        header: t("settings.keybindings.shortcut"),
        className: "data-table__actions",
        render: (item) => <KeybindingField action={item.id as KeybindingAction} />,
      },
    ],
    [query, t],
  );

  return (
    <section className="kbd-section">
      <header className="kbd-section__head">
        <h3 className="section__heading">
          {highlightMatch(t("settings.sections.keybindings"), query)}
        </h3>
        <p className="section__description">
          {highlightMatch(t("settings.keybindings.description"), query)}
        </p>
      </header>
      {items.length === 0 ? (
        <div className="settings-empty">{t("settings.searchEmpty")}</div>
      ) : (
        <Table
          columns={columns}
          rows={items}
          rowKey={(item) => item.id}
          layout="fixed"
          stickyHeader
          scrollable
        />
      )}
    </section>
  );
};
