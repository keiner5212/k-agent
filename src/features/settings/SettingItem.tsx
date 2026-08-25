import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import { Toggle } from "@/components/Toggle";
import { GlassButton } from "@/components/GlassButton";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";
import { useSettingsStore } from "@/lib/settings";
import {
  SUPPORTED_LANGUAGES,
  hardwareThreadCount,
  type AppLanguage,
  type AppTheme,
  type KeybindingAction,
  type TextScale,
} from "@/types/settings";
import type { SettingItem as SettingItemDef } from "./registry";
import { KeybindingField } from "./KeybindingField";

type SettingItemProps = {
  item: SettingItemDef;
  query: string;
};

const highlight = (text: string, query: string): ReactNode => {
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

export const SettingItem = ({ item, query }: SettingItemProps): ReactNode => {
  const { t } = useTranslation();
  const language = useSettingsStore((state) => state.language);
  const theme = useSettingsStore((state) => state.theme);
  const textScale = useSettingsStore((state) => state.textScale);
  const translucencyEnabled = useSettingsStore((state) => state.translucencyEnabled);
  const animationsEnabled = useSettingsStore((state) => state.animationsEnabled);
  const minimizeToTray = useSettingsStore((state) => state.minimizeToTray);
  const rememberWindowSize = useSettingsStore((state) => state.rememberWindowSize);
  const maxWorkerCores = useSettingsStore((state) => state.maxWorkerCores);
  const globalSkillsPath = useSettingsStore((state) => state.globalSkillsPath);
  const setLanguage = useSettingsStore((state) => state.setLanguage);
  const setTheme = useSettingsStore((state) => state.setTheme);
  const setTextScale = useSettingsStore((state) => state.setTextScale);
  const setMaxWorkerCores = useSettingsStore((state) => state.setMaxWorkerCores);
  const setTranslucencyEnabled = useSettingsStore((state) => state.setTranslucencyEnabled);
  const setAnimationsEnabled = useSettingsStore((state) => state.setAnimationsEnabled);
  const setMinimizeToTray = useSettingsStore((state) => state.setMinimizeToTray);
  const setRememberWindowSize = useSettingsStore((state) => state.setRememberWindowSize);
  const setGlobalSkillsPath = useSettingsStore((state) => state.setGlobalSkillsPath);

  const titleText = t(item.titleKey);
  const descriptionText = t(item.descriptionKey);

  return (
    <div className="setting-card">
      <div className="setting-card__head">
        <span className="setting-card__title">{highlight(titleText, query)}</span>
      </div>
      <p className="setting-card__description">{highlight(descriptionText, query)}</p>
      <div className="setting-card__control">
        {item.type === "select" ? (
          <Select
            id={`setting-${item.id}`}
            value={selectValue(item.id, {
              language,
              theme,
              textScale,
              maxWorkerCores,
            })}
            onChange={(next) =>
              onSelectChange(item.id, next, {
                setLanguage,
                setTheme,
                setTextScale,
                setMaxWorkerCores,
              })
            }
            options={selectOptions(item, t)}
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
            })}
            onChange={(next) =>
              onToggleChange(item.id, next, {
                setTranslucencyEnabled,
                setAnimationsEnabled,
                setMinimizeToTray,
                setRememberWindowSize,
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
          <GlassButton variant="ghost" onClick={() => runSettingAction(item.id)}>
            {t("settings.debug.devtools.action")}
          </GlassButton>
        ) : null}

        {item.type === "path" ? (
          <input
            id={`setting-${item.id}`}
            type="text"
            className="input"
            value={pathValue(item.id, { globalSkillsPath })}
            onChange={(event) => onPathChange(item.id, event.target.value, { setGlobalSkillsPath })}
            spellCheck={false}
            autoCorrect="off"
            autoCapitalize="off"
          />
        ) : null}
      </div>
    </div>
  );
};

type LangState = { language: AppLanguage };
type ThemeState = { theme: AppTheme };
type ScaleState = { textScale: TextScale };
type CoresState = { maxWorkerCores: number };

const selectOptions = (
  item: SettingItemDef,
  t: (key: string) => string,
): { value: string; label: string }[] => {
  if (item.id === "maxWorkerCores") {
    const options = [{ value: "0", label: t("settings.maxWorkerCores.options.auto") }];
    const max = hardwareThreadCount();
    for (let n = 1; n <= max; n += 1) {
      options.push({ value: String(n), label: String(n) });
    }
    return options;
  }
  return (item.options ?? []).map((option) => ({
    value: option.value,
    label: t(option.labelKey),
  }));
};

type ToggleState = {
  translucencyEnabled: boolean;
  animationsEnabled: boolean;
  minimizeToTray: boolean;
  rememberWindowSize: boolean;
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
  state: LangState & ThemeState & ScaleState & CoresState,
): string => {
  switch (id) {
    case "language":
      return state.language;
    case "theme":
      return state.theme;
    case "textScale":
      return String(state.textScale);
    case "maxWorkerCores":
      return String(state.maxWorkerCores);
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
    setMaxWorkerCores: (n: number) => void;
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
    case "maxWorkerCores":
      setters.setMaxWorkerCores(Number(next));
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
    default:
      return false;
  }
};

type PathState = { globalSkillsPath: string };

const pathValue = (id: string, state: PathState): string => {
  switch (id) {
    case "globalSkillsPath":
      return state.globalSkillsPath;
    default:
      return "";
  }
};

const onPathChange = (
  id: string,
  next: string,
  setters: { setGlobalSkillsPath: (path: string) => void },
): void => {
  switch (id) {
    case "globalSkillsPath":
      setters.setGlobalSkillsPath(next);
      return;
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
  }
};
