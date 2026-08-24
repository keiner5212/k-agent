import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { invoke } from "@tauri-apps/api/core";
import {
  DEFAULT_ANIMATIONS_ENABLED,
  DEFAULT_GLOBAL_SKILLS_PATH,
  DEFAULT_MAX_WORKER_CORES,
  DEFAULT_SETTINGS,
  DEFAULT_TEXT_SCALE,
  DEFAULT_TRANSLUCENCY_ENABLED,
  DEFAULT_WINDOW_BOUNDS,
  MAX_WORKER_CORES_AUTO,
  MIN_WINDOW_HEIGHT,
  MIN_WINDOW_WIDTH,
  SUPPORTED_LANGUAGES,
  TEXT_SCALE_OPTIONS,
  hardwareThreadCount,
  type AppLanguage,
  type AppTheme,
  type Keybindings,
  type Settings,
  type TextScale,
  type WindowBounds,
} from "@/types/settings";
import { isTauri } from "@/lib/platform";

const STORE_FILE = "settings.json";
const STORE_KEY = "settings";

const sanitizeLanguage = (value: unknown): AppLanguage =>
  typeof value === "string" && (SUPPORTED_LANGUAGES as readonly string[]).includes(value)
    ? (value as AppLanguage)
    : DEFAULT_SETTINGS.language;

const sanitizeTheme = (value: unknown): AppTheme =>
  value === "light" || value === "dark" ? value : DEFAULT_SETTINGS.theme;

const sanitizeBoolean = (value: unknown, fallback: boolean): boolean =>
  typeof value === "boolean" ? value : fallback;

const sanitizeTextScale = (value: unknown): TextScale => {
  if (typeof value !== "number") return DEFAULT_TEXT_SCALE;
  const match = TEXT_SCALE_OPTIONS.find((option) => Math.abs(option - value) < 0.001);
  return match ?? DEFAULT_TEXT_SCALE;
};

const sanitizeWindowBounds = (value: unknown): WindowBounds => {
  if (!value || typeof value !== "object") return DEFAULT_WINDOW_BOUNDS;
  const obj = value as Record<string, unknown>;
  const width =
    typeof obj.width === "number" && Number.isFinite(obj.width)
      ? Math.max(MIN_WINDOW_WIDTH, Math.round(obj.width))
      : DEFAULT_WINDOW_BOUNDS.width;
  const height =
    typeof obj.height === "number" && Number.isFinite(obj.height)
      ? Math.max(MIN_WINDOW_HEIGHT, Math.round(obj.height))
      : DEFAULT_WINDOW_BOUNDS.height;
  return {
    width,
    height,
    maximized: typeof obj.maximized === "boolean" ? obj.maximized : false,
  };
};

const sanitizeKeybindings = (value: unknown): Keybindings => {
  if (!value || typeof value !== "object") return DEFAULT_SETTINGS.keybindings;
  const merged: Keybindings = { ...DEFAULT_SETTINGS.keybindings };
  for (const key of Object.keys(merged) as Array<keyof Keybindings>) {
    const candidate = (value as Record<string, unknown>)[key];
    if (typeof candidate === "string" && candidate.length > 0) {
      merged[key] = candidate;
    }
  }
  return merged;
};

const sanitizeMaxWorkerCores = (value: unknown): number => {
  const max = hardwareThreadCount();
  if (value === "auto" || value === 0 || value === "0") return MAX_WORKER_CORES_AUTO;
  const n = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
  if (!Number.isFinite(n)) return DEFAULT_MAX_WORKER_CORES;
  const rounded = Math.round(n);
  if (rounded <= 0) return MAX_WORKER_CORES_AUTO;
  return Math.min(max, Math.max(1, rounded));
};

const sanitizePath = (value: unknown): string => {
  if (typeof value !== "string") return DEFAULT_GLOBAL_SKILLS_PATH;
  const trimmed = value.trim();
  if (trimmed.length === 0) return DEFAULT_GLOBAL_SKILLS_PATH;
  if (trimmed === "~/.k-agent" || trimmed === "~/.k-agent/") return DEFAULT_GLOBAL_SKILLS_PATH;
  return trimmed;
};

const sanitizeSettings = (raw: unknown): Settings => {
  if (!raw || typeof raw !== "object") return DEFAULT_SETTINGS;
  const obj = raw as Record<string, unknown>;
  return {
    language: sanitizeLanguage(obj.language),
    theme: sanitizeTheme(obj.theme),
    minimizeToTray: sanitizeBoolean(obj.minimizeToTray, DEFAULT_SETTINGS.minimizeToTray),
    translucencyEnabled: sanitizeBoolean(obj.translucencyEnabled, DEFAULT_TRANSLUCENCY_ENABLED),
    animationsEnabled: sanitizeBoolean(obj.animationsEnabled, DEFAULT_ANIMATIONS_ENABLED),
    rememberWindowSize: sanitizeBoolean(
      obj.rememberWindowSize,
      DEFAULT_SETTINGS.rememberWindowSize,
    ),
    windowBounds: sanitizeWindowBounds(obj.windowBounds),
    textScale: sanitizeTextScale(obj.textScale),
    maxWorkerCores: sanitizeMaxWorkerCores(obj.maxWorkerCores),
    keybindings: sanitizeKeybindings(obj.keybindings),
    globalSkillsPath: sanitizePath(obj.globalSkillsPath),
    sessionSidebarOpen: sanitizeBoolean(obj.sessionSidebarOpen, DEFAULT_SETTINGS.sessionSidebarOpen),
  };
};

type SettingsStore = Settings & {
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setLanguage: (language: AppLanguage) => void;
  setTheme: (theme: AppTheme) => void;
  setMinimizeToTray: (enabled: boolean) => void;
  setTranslucencyEnabled: (enabled: boolean) => void;
  setAnimationsEnabled: (enabled: boolean) => void;
  setRememberWindowSize: (enabled: boolean) => void;
  setWindowBounds: (bounds: WindowBounds) => void;
  setTextScale: (scale: TextScale) => void;
  setMaxWorkerCores: (cores: number) => void;
  setKeybinding: (action: keyof Keybindings, chord: string) => void;
  setGlobalSkillsPath: (path: string) => void;
  setSessionSidebarOpen: (open: boolean) => void;
  resetKeybindings: () => void;
  resetAll: () => void;
};

let storeHandle: LazyStore | null = null;
const getStore = (): LazyStore => {
  if (!storeHandle) storeHandle = new LazyStore(STORE_FILE);
  return storeHandle;
};

const persist = async (next: Settings): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(STORE_KEY, next);
    await getStore().save();
  } catch (error) {
    console.warn("settings persist failed", error);
  }
};

const applyTheme = (theme: AppTheme): void => {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = theme;
};

const applyTranslucency = (enabled: boolean): void => {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.translucent = enabled ? "true" : "false";
};

const applyAnimations = (enabled: boolean): void => {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.animations = enabled ? "enabled" : "disabled";
};

const applyTextScale = (scale: TextScale): void => {
  if (typeof document === "undefined") return;
  document.documentElement.style.setProperty("--text-scale", String(scale));
};

const systemPrefersReducedMotion = (): boolean => {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
};

const applyChrome = (settings: Settings): void => {
  applyTheme(settings.theme);
  applyTranslucency(settings.translucencyEnabled);
  applyAnimations(settings.animationsEnabled && !systemPrefersReducedMotion());
  applyTextScale(settings.textScale);
};

const syncMinimizeToTray = async (enabled: boolean): Promise<void> => {
  if (!isTauri()) return;
  try {
    await invoke("set_minimize_to_tray", { enabled });
  } catch (error) {
    console.warn("tray sync failed", error);
  }
};

const snapshot = (state: SettingsStore): Settings => ({
  language: state.language,
  theme: state.theme,
  minimizeToTray: state.minimizeToTray,
  translucencyEnabled: state.translucencyEnabled,
  animationsEnabled: state.animationsEnabled,
  rememberWindowSize: state.rememberWindowSize,
  windowBounds: state.windowBounds,
  textScale: state.textScale,
  maxWorkerCores: state.maxWorkerCores,
  keybindings: state.keybindings,
  globalSkillsPath: state.globalSkillsPath,
  sessionSidebarOpen: state.sessionSidebarOpen,
});

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  ...DEFAULT_SETTINGS,
  hydrated: false,

  hydrate: async () => {
    if (!isTauri()) {
      applyChrome(DEFAULT_SETTINGS);
      set({ hydrated: true });
      return;
    }
    try {
      const raw = await getStore().get<Settings>(STORE_KEY);
      const next = sanitizeSettings(raw);
      applyChrome(next);
      void syncMinimizeToTray(next.minimizeToTray);
      set({ ...next, hydrated: true });
    } catch (error) {
      console.warn("settings hydrate failed", error);
      applyChrome(DEFAULT_SETTINGS);
      set({ hydrated: true });
    }
  },

  setLanguage: (language) => {
    set({ language });
    void persist(snapshot(get()));
  },

  setTheme: (theme) => {
    applyTheme(theme);
    set({ theme });
    void persist(snapshot(get()));
  },

  setMinimizeToTray: (enabled) => {
    set({ minimizeToTray: enabled });
    void syncMinimizeToTray(enabled);
    void persist(snapshot(get()));
  },

  setTranslucencyEnabled: (enabled) => {
    applyTranslucency(enabled);
    set({ translucencyEnabled: enabled });
    void persist(snapshot(get()));
  },

  setAnimationsEnabled: (enabled) => {
    applyAnimations(enabled && !systemPrefersReducedMotion());
    set({ animationsEnabled: enabled });
    void persist(snapshot(get()));
  },

  setRememberWindowSize: (enabled) => {
    set({ rememberWindowSize: enabled });
    if (enabled && isTauri()) {
      void invoke<WindowBounds>("window_get_bounds")
        .then((next) => {
          set({ windowBounds: sanitizeWindowBounds(next) });
          void persist(snapshot(get()));
        })
        .catch(() => {
          void persist(snapshot(get()));
        });
      return;
    }
    void persist(snapshot(get()));
  },

  setWindowBounds: (bounds) => {
    set({ windowBounds: sanitizeWindowBounds(bounds) });
    void persist(snapshot(get()));
  },

  setTextScale: (scale) => {
    const textScale = sanitizeTextScale(scale);
    applyTextScale(textScale);
    set({ textScale });
    void persist(snapshot(get()));
  },

  setMaxWorkerCores: (cores) => {
    set({ maxWorkerCores: sanitizeMaxWorkerCores(cores) });
    void persist(snapshot(get()));
  },

  setKeybinding: (action, chord) => {
    const keybindings = { ...get().keybindings, [action]: chord };
    set({ keybindings });
    void persist(snapshot(get()));
  },

  setGlobalSkillsPath: (path) => {
    set({ globalSkillsPath: sanitizePath(path) });
    void persist(snapshot(get()));
  },

  setSessionSidebarOpen: (open) => {
    set({ sessionSidebarOpen: open });
    void persist(snapshot(get()));
  },

  resetKeybindings: () => {
    set({ keybindings: DEFAULT_SETTINGS.keybindings });
    void persist(snapshot(get()));
  },

  resetAll: () => {
    applyChrome(DEFAULT_SETTINGS);
    void syncMinimizeToTray(DEFAULT_SETTINGS.minimizeToTray);
    set({ ...DEFAULT_SETTINGS, hydrated: true });
    void persist(DEFAULT_SETTINGS);
  },
}));
