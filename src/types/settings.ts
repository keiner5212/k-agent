export type AppLanguage = "en" | "es";
export type AppTheme = "dark" | "light";

export const SUPPORTED_LANGUAGES: readonly AppLanguage[] = ["en", "es"] as const;
export const DEFAULT_LANGUAGE: AppLanguage = "en";
export const DEFAULT_THEME: AppTheme = "dark";
export const DEFAULT_MINIMIZE_TO_TRAY = false;
export const DEFAULT_TRANSLUCENCY_ENABLED = true;

export type KeybindingAction = "settings.open" | "settings.close";

export type Keybindings = Record<KeybindingAction, string>;

export const DEFAULT_KEYBINDINGS: Keybindings = {
  "settings.open": "Mod+P",
  "settings.close": "Escape",
};

export type Settings = {
  language: AppLanguage;
  theme: AppTheme;
  minimizeToTray: boolean;
  translucencyEnabled: boolean;
  keybindings: Keybindings;
};

export const DEFAULT_SETTINGS: Settings = {
  language: DEFAULT_LANGUAGE,
  theme: DEFAULT_THEME,
  minimizeToTray: DEFAULT_MINIMIZE_TO_TRAY,
  translucencyEnabled: DEFAULT_TRANSLUCENCY_ENABLED,
  keybindings: DEFAULT_KEYBINDINGS,
};
