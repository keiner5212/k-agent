import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { invoke } from "@tauri-apps/api/core";
import {
  DEFAULT_ANIMATIONS_ENABLED,
  DEFAULT_SETTINGS,
  DEFAULT_TRANSLUCENCY_ENABLED,
  SUPPORTED_LANGUAGES,
  type AppLanguage,
  type AppTheme,
  type Keybindings,
  type Settings,
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

const sanitizeSettings = (raw: unknown): Settings => {
  if (!raw || typeof raw !== "object") return DEFAULT_SETTINGS;
  const obj = raw as Record<string, unknown>;
  return {
    language: sanitizeLanguage(obj.language),
    theme: sanitizeTheme(obj.theme),
    minimizeToTray: sanitizeBoolean(obj.minimizeToTray, DEFAULT_SETTINGS.minimizeToTray),
    translucencyEnabled: sanitizeBoolean(obj.translucencyEnabled, DEFAULT_TRANSLUCENCY_ENABLED),
    animationsEnabled: sanitizeBoolean(obj.animationsEnabled, DEFAULT_ANIMATIONS_ENABLED),
    keybindings: sanitizeKeybindings(obj.keybindings),
  };
};

type Persistable = Pick<
  Settings,
  | "language"
  | "theme"
  | "minimizeToTray"
  | "translucencyEnabled"
  | "animationsEnabled"
  | "keybindings"
>;

type SettingsStore = Settings & {
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setLanguage: (language: AppLanguage) => void;
  setTheme: (theme: AppTheme) => void;
  setMinimizeToTray: (enabled: boolean) => void;
  setTranslucencyEnabled: (enabled: boolean) => void;
  setAnimationsEnabled: (enabled: boolean) => void;
  setKeybinding: (action: keyof Keybindings, chord: string) => void;
  resetKeybindings: () => void;
  resetAll: () => void;
};

let storeHandle: LazyStore | null = null;
const getStore = (): LazyStore => {
  if (!storeHandle) storeHandle = new LazyStore(STORE_FILE);
  return storeHandle;
};

const persist = async (next: Persistable): Promise<void> => {
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

const systemPrefersReducedMotion = (): boolean => {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
};

const syncMinimizeToTray = async (enabled: boolean): Promise<void> => {
  if (!isTauri()) return;
  try {
    await invoke("set_minimize_to_tray", { enabled });
  } catch (error) {
    console.warn("tray sync failed", error);
  }
};

const snapshot = (state: SettingsStore): Persistable => ({
  language: state.language,
  theme: state.theme,
  minimizeToTray: state.minimizeToTray,
  translucencyEnabled: state.translucencyEnabled,
  animationsEnabled: state.animationsEnabled,
  keybindings: state.keybindings,
});

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  ...DEFAULT_SETTINGS,
  hydrated: false,

  hydrate: async () => {
    if (!isTauri()) {
      applyTheme(DEFAULT_SETTINGS.theme);
      applyTranslucency(DEFAULT_TRANSLUCENCY_ENABLED);
      applyAnimations(DEFAULT_ANIMATIONS_ENABLED && !systemPrefersReducedMotion());
      set({ hydrated: true });
      return;
    }
    try {
      const raw = await getStore().get<Settings>(STORE_KEY);
      const next = sanitizeSettings(raw);
      applyTheme(next.theme);
      applyTranslucency(next.translucencyEnabled);
      applyAnimations(next.animationsEnabled && !systemPrefersReducedMotion());
      void syncMinimizeToTray(next.minimizeToTray);
      set({ ...next, hydrated: true });
    } catch (error) {
      console.warn("settings hydrate failed", error);
      applyTheme(DEFAULT_SETTINGS.theme);
      applyTranslucency(DEFAULT_TRANSLUCENCY_ENABLED);
      applyAnimations(DEFAULT_ANIMATIONS_ENABLED && !systemPrefersReducedMotion());
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

  setKeybinding: (action, chord) => {
    const keybindings = { ...get().keybindings, [action]: chord };
    set({ keybindings });
    void persist(snapshot(get()));
  },

  resetKeybindings: () => {
    set({ keybindings: DEFAULT_SETTINGS.keybindings });
    void persist(snapshot(get()));
  },

  resetAll: () => {
    applyTheme(DEFAULT_SETTINGS.theme);
    applyTranslucency(DEFAULT_TRANSLUCENCY_ENABLED);
    applyAnimations(DEFAULT_ANIMATIONS_ENABLED && !systemPrefersReducedMotion());
    void syncMinimizeToTray(DEFAULT_SETTINGS.minimizeToTray);
    set({ ...DEFAULT_SETTINGS, hydrated: true });
    void persist(DEFAULT_SETTINGS);
  },
}));
