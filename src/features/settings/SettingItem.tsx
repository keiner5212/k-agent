import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import { Toggle } from "@/components/Toggle";
import { GlassButton } from "@/components/GlassButton";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";
import { highlightMatch } from "@/lib/highlight";
import { useSettingsStore } from "@/lib/settings";
import { useProvidersStore } from "@/lib/providers";
import { listSystemFonts } from "@/lib/system-fonts";
import {
  FONT_FAMILY_OPTIONS,
  REMINDER_INTERVAL_OPTIONS,
  SUPPORTED_LANGUAGES,
  hardwareThreadCount,
  type AppFontFamily,
  type AppLanguage,
  type AppTheme,
  type KeybindingAction,
  type TextScale,
} from "@/types/settings";
import type { SelectedModel } from "@/types/chat";
import type { Provider } from "@/types/providers";
import type { SettingItem as SettingItemDef } from "./registry";
import { KeybindingField } from "./KeybindingField";
import { ListEditor } from "./ListEditor";

type SettingItemProps = {
  item: SettingItemDef;
  query: string;
};

export const SettingItem = ({ item, query }: SettingItemProps): ReactNode => {
  const { t } = useTranslation();
  const language = useSettingsStore((state) => state.language);
  const theme = useSettingsStore((state) => state.theme);
  const textScale = useSettingsStore((state) => state.textScale);
  const fontFamily = useSettingsStore((state) => state.fontFamily);
  const translucencyEnabled = useSettingsStore((state) => state.translucencyEnabled);
  const animationsEnabled = useSettingsStore((state) => state.animationsEnabled);
  const minimizeToTray = useSettingsStore((state) => state.minimizeToTray);
  const rememberWindowSize = useSettingsStore((state) => state.rememberWindowSize);
  const maxWorkerCores = useSettingsStore((state) => state.maxWorkerCores);
  const reminderInterval = useSettingsStore((state) => state.reminderInterval);
  const forceResponseLanguage = useSettingsStore((state) => state.forceResponseLanguage);
  const responseLanguage = useSettingsStore((state) => state.responseLanguage);
  const blockedCommands = useSettingsStore((state) => state.blockedCommands);
  const allowedCommands = useSettingsStore((state) => state.allowedCommands);
  const notificationsEnabled = useSettingsStore((state) => state.notificationsEnabled);
  const taskCompleteSoundEnabled = useSettingsStore((state) => state.taskCompleteSoundEnabled);
  const workspaceMemoryEnabled = useSettingsStore((state) => state.workspaceMemoryEnabled);
  const buildAgentEnabled = useSettingsStore((state) => state.buildAgentEnabled);
  const planAgentEnabled = useSettingsStore((state) => state.planAgentEnabled);
  const titleGenerationModel = useSettingsStore((state) => state.titleGenerationModel);
  const titleUseFirstMessage = useSettingsStore((state) => state.titleUseFirstMessage);
  const appGenerationModel = useSettingsStore((state) => state.appGenerationModel);
  const lspEnabled = useSettingsStore((state) => state.lspEnabled);
  const setLanguage = useSettingsStore((state) => state.setLanguage);
  const setTheme = useSettingsStore((state) => state.setTheme);
  const setTextScale = useSettingsStore((state) => state.setTextScale);
  const setFontFamily = useSettingsStore((state) => state.setFontFamily);
  const setMaxWorkerCores = useSettingsStore((state) => state.setMaxWorkerCores);
  const setReminderInterval = useSettingsStore((state) => state.setReminderInterval);
  const setForceResponseLanguage = useSettingsStore((state) => state.setForceResponseLanguage);
  const setResponseLanguage = useSettingsStore((state) => state.setResponseLanguage);
  const setBlockedCommands = useSettingsStore((state) => state.setBlockedCommands);
  const setAllowedCommands = useSettingsStore((state) => state.setAllowedCommands);
  const setNotificationsEnabled = useSettingsStore((state) => state.setNotificationsEnabled);
  const setTaskCompleteSoundEnabled = useSettingsStore(
    (state) => state.setTaskCompleteSoundEnabled,
  );
  const setWorkspaceMemoryEnabled = useSettingsStore((state) => state.setWorkspaceMemoryEnabled);
  const setBuildAgentEnabled = useSettingsStore((state) => state.setBuildAgentEnabled);
  const setPlanAgentEnabled = useSettingsStore((state) => state.setPlanAgentEnabled);
  const setTitleGenerationModel = useSettingsStore((state) => state.setTitleGenerationModel);
  const setTitleUseFirstMessage = useSettingsStore((state) => state.setTitleUseFirstMessage);
  const setAppGenerationModel = useSettingsStore((state) => state.setAppGenerationModel);
  const setLspEnabled = useSettingsStore((state) => state.setLspEnabled);
  const providers = useProvidersStore((state) => state.providers);
  const providersLoading = useProvidersStore((state) => state.loading);
  const loadProviders = useProvidersStore((state) => state.load);
  const setTranslucencyEnabled = useSettingsStore((state) => state.setTranslucencyEnabled);
  const setAnimationsEnabled = useSettingsStore((state) => state.setAnimationsEnabled);
  const setMinimizeToTray = useSettingsStore((state) => state.setMinimizeToTray);
  const setRememberWindowSize = useSettingsStore((state) => state.setRememberWindowSize);
  const [systemFonts, setSystemFonts] = useState<string[]>([]);

  useEffect(() => {
    if (item.id !== "fontFamily") return;
    let cancelled = false;
    void listSystemFonts().then((names) => {
      if (!cancelled) setSystemFonts(names);
    });
    return () => {
      cancelled = true;
    };
  }, [item.id]);

  useEffect(() => {
    if (item.type !== "modelChoice") return;
    if (providers.length === 0 && !providersLoading) {
      void loadProviders();
    }
  }, [item.type, providers.length, providersLoading, loadProviders]);

  const titleText = t(item.titleKey);
  const descriptionText = t(item.descriptionKey);

  return (
    <div className="setting-card">
      <div className="setting-card__head">
        <span className="setting-card__title">{highlightMatch(titleText, query)}</span>
      </div>
      <p className="setting-card__description">{highlightMatch(descriptionText, query)}</p>
      <div className="setting-card__control">
        {item.type === "select" ? (
          <Select
            id={`setting-${item.id}`}
            value={selectValue(item.id, {
              language,
              theme,
              textScale,
              fontFamily,
              maxWorkerCores,
              reminderInterval,
              responseLanguage,
            })}
            onChange={(next) =>
              onSelectChange(item.id, next, {
                setLanguage,
                setTheme,
                setTextScale,
                setFontFamily,
                setMaxWorkerCores,
                setReminderInterval,
                setResponseLanguage,
              })
            }
            options={selectOptions(item, t, { systemFonts, fontFamily })}
          />
        ) : null}

        {item.type === "modelChoice" ? (
          <Select
            id={`setting-${item.id}`}
            value={modelChoiceValue(item.id, { titleGenerationModel, appGenerationModel })}
            onChange={(next) =>
              onModelChoiceChange(item.id, next, {
                setTitleGenerationModel,
                setAppGenerationModel,
              })
            }
            options={modelChoiceOptions(item, t, providers)}
          />
        ) : null}

        {item.type === "toggle" ? (
          <Toggle
            showLabel={false}
            checked={toggleChecked(item.id, {
              translucencyEnabled,
              animationsEnabled,
              minimizeToTray,
              rememberWindowSize,
              forceResponseLanguage,
              notificationsEnabled,
              taskCompleteSoundEnabled,
              workspaceMemoryEnabled,
              buildAgentEnabled,
              planAgentEnabled,
              titleUseFirstMessage,
              lspEnabled,
            })}
            onChange={(next) =>
              onToggleChange(item.id, next, {
                setTranslucencyEnabled,
                setAnimationsEnabled,
                setMinimizeToTray,
                setRememberWindowSize,
                setForceResponseLanguage,
                setNotificationsEnabled,
                setTaskCompleteSoundEnabled,
                setWorkspaceMemoryEnabled,
                setBuildAgentEnabled,
                setPlanAgentEnabled,
                setTitleUseFirstMessage,
                setLspEnabled,
              })
            }
            label={titleText}
            description={descriptionText}
          />
        ) : null}

        {item.type === "keybinding" ? (
          <KeybindingField action={item.id as KeybindingAction} />
        ) : null}

        {item.type === "action" ? (
          <GlassButton variant="secondary" onClick={() => runSettingAction(item.id)}>
            {t("settings.debug.devtools.action")}
          </GlassButton>
        ) : null}

        {item.type === "list" ? (
          <ListEditor
            id={`setting-${item.id}`}
            value={listValue(item.id, { blockedCommands, allowedCommands })}
            onChange={(next) =>
              onListChange(item.id, next, { setBlockedCommands, setAllowedCommands })
            }
            placeholder={t(`${listKeys(item.id)}.placeholder`)}
            addLabel={t(`${listKeys(item.id)}.add`)}
            emptyMessage={t(`${listKeys(item.id)}.empty`)}
            removeLabel={t(`${listKeys(item.id)}.remove`)}
          />
        ) : null}
      </div>
    </div>
  );
};

type LangState = { language: AppLanguage };
type ThemeState = { theme: AppTheme };
type ScaleState = { textScale: TextScale };
type FontState = { fontFamily: AppFontFamily };
type CoresState = { maxWorkerCores: number };
type ReminderState = { reminderInterval: number };
type ResponseLangState = { responseLanguage: AppLanguage };

const selectOptions = (
  item: SettingItemDef,
  t: (key: string) => string,
  extras: { systemFonts: string[]; fontFamily: string },
): { value: string; label: ReactNode }[] => {
  if (item.id === "maxWorkerCores") {
    const options = [{ value: "0", label: t("settings.maxWorkerCores.options.auto") }];
    const max = hardwareThreadCount();
    for (let n = 1; n <= max; n += 1) {
      options.push({ value: String(n), label: String(n) });
    }
    return options;
  }
  if (item.id === "reminderInterval") {
    return REMINDER_INTERVAL_OPTIONS.map((value) => ({
      value: String(value),
      label: String(value),
    }));
  }
  if (item.id === "fontFamily") {
    return fontFamilyOptions(t, extras.systemFonts, extras.fontFamily);
  }
  return (item.options ?? []).map((option) => ({
    value: option.value,
    label: t(option.labelKey),
  }));
};

const fontFamilyOptions = (
  t: (key: string) => string,
  systemFonts: string[],
  current: string,
): { value: string; label: ReactNode }[] => {
  const presetValues = new Set<string>(FONT_FAMILY_OPTIONS);
  const options: { value: string; label: ReactNode }[] = FONT_FAMILY_OPTIONS.map((value) => ({
    value,
    label: t(`settings.fontFamily.options.${value}`),
  }));
  for (const name of systemFonts) {
    if (presetValues.has(name)) continue;
    options.push({
      value: name,
      label: <span style={{ fontFamily: `"${name}", sans-serif` }}>{name}</span>,
    });
  }
  if (current.length > 0 && !options.some((option) => option.value === current)) {
    options.push({
      value: current,
      label: <span style={{ fontFamily: `"${current}", sans-serif` }}>{current}</span>,
    });
  }
  return options;
};

type ToggleState = {
  translucencyEnabled: boolean;
  animationsEnabled: boolean;
  minimizeToTray: boolean;
  rememberWindowSize: boolean;
  forceResponseLanguage: boolean;
  notificationsEnabled: boolean;
  taskCompleteSoundEnabled: boolean;
  workspaceMemoryEnabled: boolean;
  buildAgentEnabled: boolean;
  planAgentEnabled: boolean;
  titleUseFirstMessage: boolean;
  lspEnabled: boolean;
};

const runSettingAction = (id: string): void => {
  if (id !== "openDevtools") return;
  if (!isTauri()) {
    console.warn("window_open_devtools skipped: not running in Tauri");
    return;
  }
  void invoke("window_open_devtools").catch((error: unknown) => {
    console.warn("window_open_devtools failed", error);
  });
};

const selectValue = (
  id: string,
  state: LangState &
    ThemeState &
    ScaleState &
    FontState &
    CoresState &
    ReminderState &
    ResponseLangState,
): string => {
  switch (id) {
    case "language":
      return state.language;
    case "theme":
      return state.theme;
    case "textScale":
      return String(state.textScale);
    case "fontFamily":
      return state.fontFamily;
    case "maxWorkerCores":
      return String(state.maxWorkerCores);
    case "reminderInterval":
      return String(state.reminderInterval);
    case "responseLanguage":
      return state.responseLanguage;
    default:
      return "";
  }
};

const onSelectChange = (
  id: string,
  next: string,
  setters: {
    setLanguage: (l: AppLanguage) => void;
    setTheme: (t: AppTheme) => void;
    setTextScale: (s: TextScale) => void;
    setFontFamily: (f: AppFontFamily) => void;
    setMaxWorkerCores: (n: number) => void;
    setReminderInterval: (n: number) => void;
    setResponseLanguage: (l: AppLanguage) => void;
  },
): void => {
  switch (id) {
    case "language":
      if ((SUPPORTED_LANGUAGES as readonly string[]).includes(next)) {
        setters.setLanguage(next as AppLanguage);
      }
      return;
    case "theme":
      setters.setTheme(next as AppTheme);
      return;
    case "textScale":
      setters.setTextScale(Number(next) as TextScale);
      return;
    case "fontFamily":
      setters.setFontFamily(next);
      return;
    case "maxWorkerCores":
      setters.setMaxWorkerCores(Number(next));
      return;
    case "reminderInterval":
      setters.setReminderInterval(Number(next));
      return;
    case "responseLanguage":
      if ((SUPPORTED_LANGUAGES as readonly string[]).includes(next)) {
        setters.setResponseLanguage(next as AppLanguage);
      }
      return;
  }
};

const toggleChecked = (id: string, state: ToggleState): boolean => {
  switch (id) {
    case "translucencyEnabled":
      return !state.translucencyEnabled;
    case "animationsEnabled":
      return !state.animationsEnabled;
    case "minimizeToTray":
      return state.minimizeToTray;
    case "rememberWindowSize":
      return state.rememberWindowSize;
    case "forceResponseLanguage":
      return state.forceResponseLanguage;
    case "notificationsEnabled":
      return state.notificationsEnabled;
    case "taskCompleteSoundEnabled":
      return state.taskCompleteSoundEnabled;
    case "workspaceMemoryEnabled":
      return state.workspaceMemoryEnabled;
    case "buildAgentEnabled":
      return state.buildAgentEnabled;
    case "planAgentEnabled":
      return state.planAgentEnabled;
    case "titleUseFirstMessage":
      return state.titleUseFirstMessage;
    case "lspEnabled":
      return state.lspEnabled;
    default:
      return false;
  }
};

const onToggleChange = (
  id: string,
  next: boolean,
  setters: {
    setTranslucencyEnabled: (v: boolean) => void;
    setAnimationsEnabled: (v: boolean) => void;
    setMinimizeToTray: (v: boolean) => void;
    setRememberWindowSize: (v: boolean) => void;
    setForceResponseLanguage: (v: boolean) => void;
    setNotificationsEnabled: (v: boolean) => void;
    setTaskCompleteSoundEnabled: (v: boolean) => void;
    setWorkspaceMemoryEnabled: (v: boolean) => void;
    setBuildAgentEnabled: (v: boolean) => void;
    setPlanAgentEnabled: (v: boolean) => void;
    setTitleUseFirstMessage: (v: boolean) => void;
    setLspEnabled: (v: boolean) => void;
  },
): void => {
  switch (id) {
    case "translucencyEnabled":
      setters.setTranslucencyEnabled(!next);
      return;
    case "animationsEnabled":
      setters.setAnimationsEnabled(!next);
      return;
    case "minimizeToTray":
      setters.setMinimizeToTray(next);
      return;
    case "rememberWindowSize":
      setters.setRememberWindowSize(next);
      return;
    case "forceResponseLanguage":
      setters.setForceResponseLanguage(next);
      return;
    case "notificationsEnabled":
      setters.setNotificationsEnabled(next);
      return;
    case "taskCompleteSoundEnabled":
      setters.setTaskCompleteSoundEnabled(next);
      return;
    case "workspaceMemoryEnabled":
      setters.setWorkspaceMemoryEnabled(next);
      return;
    case "buildAgentEnabled":
      setters.setBuildAgentEnabled(next);
      return;
    case "planAgentEnabled":
      setters.setPlanAgentEnabled(next);
      return;
    case "titleUseFirstMessage":
      setters.setTitleUseFirstMessage(next);
      return;
    case "lspEnabled":
      setters.setLspEnabled(next);
      return;
  }
};

type ListValueState = { blockedCommands: string[]; allowedCommands: string[] };

const listValue = (id: string, state: ListValueState): string[] => {
  switch (id) {
    case "blockedCommands":
      return state.blockedCommands;
    case "allowedCommands":
      return state.allowedCommands;
    default:
      return [];
  }
};

const onListChange = (
  id: string,
  next: string[],
  setters: { setBlockedCommands: (v: string[]) => void; setAllowedCommands: (v: string[]) => void },
): void => {
  switch (id) {
    case "blockedCommands":
      setters.setBlockedCommands(next);
      return;
    case "allowedCommands":
      setters.setAllowedCommands(next);
      return;
  }
};

const listKeys = (id: string): string => `settings.${id}`;

type ModelChoiceState = {
  titleGenerationModel: SelectedModel | null;
  appGenerationModel: SelectedModel | null;
};

const encodeModel = (model: SelectedModel | null): string => {
  if (!model) return "";
  return `${model.providerId}::${model.modelId}`;
};

const decodeModel = (value: string): SelectedModel | null => {
  if (value.length === 0) return null;
  const idx = value.indexOf("::");
  if (idx <= 0 || idx >= value.length - 2) return null;
  const providerId = value.slice(0, idx);
  const modelId = value.slice(idx + 2);
  if (providerId.length === 0 || modelId.length === 0) return null;
  return { providerId, modelId };
};

const modelChoiceValue = (id: string, state: ModelChoiceState): string => {
  switch (id) {
    case "titleGenerationModel":
      return encodeModel(state.titleGenerationModel);
    case "appGenerationModel":
      return encodeModel(state.appGenerationModel);
    default:
      return "";
  }
};

const onModelChoiceChange = (
  id: string,
  next: string,
  setters: {
    setTitleGenerationModel: (m: SelectedModel | null) => void;
    setAppGenerationModel: (m: SelectedModel | null) => void;
  },
): void => {
  const model = decodeModel(next);
  switch (id) {
    case "titleGenerationModel":
      setters.setTitleGenerationModel(model);
      return;
    case "appGenerationModel":
      setters.setAppGenerationModel(model);
      return;
  }
};

const modelChoiceOptions = (
  item: SettingItemDef,
  t: (key: string) => string,
  providers: Provider[],
): { value: string; label: ReactNode }[] => {
  const options: { value: string; label: ReactNode }[] = [
    { value: "", label: t(`${listKeys(item.id)}.options.useChatModel`) },
  ];
  for (const provider of providers) {
    for (const model of provider.models) {
      options.push({
        value: encodeModel({ providerId: provider.id, modelId: model.id }),
        label: `${provider.name} / ${model.displayName ?? model.id}`,
      });
    }
  }
  return options;
};
