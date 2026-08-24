import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Select } from "@/components/Select";
import { Toggle } from "@/components/Toggle";
import { useSettingsStore } from "@/lib/settings";
import {
  SUPPORTED_LANGUAGES,
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
  const setLanguage = useSettingsStore((state) => state.setLanguage);
  const setTheme = useSettingsStore((state) => state.setTheme);
  const setTextScale = useSettingsStore((state) => state.setTextScale);
  const setTranslucencyEnabled = useSettingsStore((state) => state.setTranslucencyEnabled);
  const setAnimationsEnabled = useSettingsStore((state) => state.setAnimationsEnabled);
  const setMinimizeToTray = useSettingsStore((state) => state.setMinimizeToTray);
  const setRememberWindowSize = useSettingsStore((state) => state.setRememberWindowSize);

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
            })}
            onChange={(next) =>
              onSelectChange(item.id, next, {
                setLanguage,
                setTheme,
                setTextScale,
              })
            }
            options={(item.options ?? []).map((option) => ({
              value: option.value,
              label: t(option.labelKey),
            }))}
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
      </div>
    </div>
  );
};

type LangState = { language: AppLanguage };
type ThemeState = { theme: AppTheme };
type ScaleState = { textScale: TextScale };
type ToggleState = {
  translucencyEnabled: boolean;
  animationsEnabled: boolean;
  minimizeToTray: boolean;
  rememberWindowSize: boolean;
};

const selectValue = (id: string, state: LangState & ThemeState & ScaleState): string => {
  switch (id) {
    case "language":
      return state.language;
    case "theme":
      return state.theme;
    case "textScale":
      return String(state.textScale);
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
  }
};

const toggleChecked = (id: string, state: ToggleState): boolean => {
  switch (id) {
    case "translucency":
      return !state.translucencyEnabled;
    case "animations":
      return !state.animationsEnabled;
    case "minimizeToTray":
      return state.minimizeToTray;
    case "rememberWindowSize":
      return state.rememberWindowSize;
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
  },
): void => {
  switch (id) {
    case "translucency":
      setters.setTranslucencyEnabled(!next);
      return;
    case "animations":
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
