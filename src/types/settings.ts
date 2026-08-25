export type AppLanguage = "en" | "es";
export type AppTheme = "dark" | "light";

export const SUPPORTED_LANGUAGES: readonly AppLanguage[] = ["en", "es"] as const;
export const DEFAULT_LANGUAGE: AppLanguage = "en";
export const DEFAULT_THEME: AppTheme = "dark";
export const DEFAULT_MINIMIZE_TO_TRAY = false;
export const DEFAULT_TRANSLUCENCY_ENABLED = true;
export const DEFAULT_ANIMATIONS_ENABLED = true;

export type TextScale = 0.5 | 0.6 | 0.7 | 0.8 | 0.875 | 0.92 | 1 | 1.125 | 1.25 | 1.4;

export const TEXT_SCALE_OPTIONS: readonly TextScale[] = [
  0.5, 0.6, 0.7, 0.8, 0.875, 0.92, 1, 1.125, 1.25, 1.4,
] as const;

export const DEFAULT_TEXT_SCALE: TextScale = 1;

export type AppFontFamily = string;

export const FONT_FAMILY_OPTIONS = ["system", "humanist", "geometric", "serif", "mono"] as const;

export const DEFAULT_FONT_FAMILY: AppFontFamily = "system";

export const DEFAULT_REMEMBER_WINDOW_SIZE = true;
export const DEFAULT_WINDOW_WIDTH = 1100;
export const DEFAULT_WINDOW_HEIGHT = 720;
export const MIN_WINDOW_WIDTH = 720;
export const MIN_WINDOW_HEIGHT = 480;

export const MAX_WORKER_CORES_AUTO = 0;
export const DEFAULT_MAX_WORKER_CORES = MAX_WORKER_CORES_AUTO;
export const MAX_WORKER_CORES_CAP = 256;

export const DEFAULT_REMINDER_INTERVAL = 8;
export const MIN_REMINDER_INTERVAL = 1;
export const MAX_REMINDER_INTERVAL = 500;

export const REMINDER_INTERVAL_OPTIONS = [4, 6, 8, 10, 12, 16, 20, 30, 50] as const;

export const DEFAULT_FORCE_RESPONSE_LANGUAGE = false;
export const DEFAULT_RESPONSE_LANGUAGE: AppLanguage = DEFAULT_LANGUAGE;

export const DEFAULT_SESSION_SIDEBAR_OPEN = true;

export const hardwareThreadCount = (): number => {
  if (typeof navigator === "undefined") return 8;
  const n = navigator.hardwareConcurrency;
  if (typeof n !== "number" || !Number.isFinite(n) || n < 1) return 8;
  return Math.min(MAX_WORKER_CORES_CAP, Math.floor(n));
};

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

export type KeybindingAction =
  "settings.open" | "settings.close" | "sidebar.toggle" | "chat.clear" | "editor.save";

export type Keybindings = Record<KeybindingAction, string>;

export const DEFAULT_KEYBINDINGS: Keybindings = {
  "settings.open": "Ctrl+P",
  "settings.close": "Escape",
  "sidebar.toggle": "Ctrl+B",
  "chat.clear": "Alt+C",
  "editor.save": "Ctrl+S",
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
  fontFamily: AppFontFamily;
  maxWorkerCores: number;
  reminderInterval: number;
  forceResponseLanguage: boolean;
  responseLanguage: AppLanguage;
  keybindings: Keybindings;
  sessionSidebarOpen: boolean;
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
  fontFamily: DEFAULT_FONT_FAMILY,
  maxWorkerCores: DEFAULT_MAX_WORKER_CORES,
  reminderInterval: DEFAULT_REMINDER_INTERVAL,
  forceResponseLanguage: DEFAULT_FORCE_RESPONSE_LANGUAGE,
  responseLanguage: DEFAULT_RESPONSE_LANGUAGE,
  keybindings: DEFAULT_KEYBINDINGS,
  sessionSidebarOpen: DEFAULT_SESSION_SIDEBAR_OPEN,
};
