export type AppLanguage = "en" | "es";
export type AppTheme = "dark" | "light";

export const SUPPORTED_LANGUAGES: readonly AppLanguage[] = ["en", "es"] as const;
export const DEFAULT_LANGUAGE: AppLanguage = "en";
export const DEFAULT_THEME: AppTheme = "dark";
export const DEFAULT_MINIMIZE_TO_TRAY = false;
export const DEFAULT_TRANSLUCENCY_ENABLED = true;
export const DEFAULT_ANIMATIONS_ENABLED = true;

export type TextScale = 0.875 | 0.92 | 1 | 1.125 | 1.25;

export const TEXT_SCALE_OPTIONS: readonly TextScale[] = [0.875, 0.92, 1, 1.125, 1.25] as const;

export const DEFAULT_TEXT_SCALE: TextScale = 1;

export const DEFAULT_REMEMBER_WINDOW_SIZE = true;
export const DEFAULT_WINDOW_WIDTH = 1100;
export const DEFAULT_WINDOW_HEIGHT = 720;
export const MIN_WINDOW_WIDTH = 720;
export const MIN_WINDOW_HEIGHT = 480;

export type WindowBounds = {
  width: number;
  height: number;
  maximized: boolean;
};

export const DEFAULT_WINDOW_BOUNDS: WindowBounds = {
  width: DEFAULT_WINDOW_WIDTH,
  height: DEFAULT_WINDOW_HEIGHT,
  maximized: false,
};

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
  animationsEnabled: boolean;
  rememberWindowSize: boolean;
  windowBounds: WindowBounds;
  textScale: TextScale;
  keybindings: Keybindings;
};

export const DEFAULT_SETTINGS: Settings = {
  language: DEFAULT_LANGUAGE,
  theme: DEFAULT_THEME,
  minimizeToTray: DEFAULT_MINIMIZE_TO_TRAY,
  translucencyEnabled: DEFAULT_TRANSLUCENCY_ENABLED,
  animationsEnabled: DEFAULT_ANIMATIONS_ENABLED,
  rememberWindowSize: DEFAULT_REMEMBER_WINDOW_SIZE,
  windowBounds: DEFAULT_WINDOW_BOUNDS,
  textScale: DEFAULT_TEXT_SCALE,
  keybindings: DEFAULT_KEYBINDINGS,
};
