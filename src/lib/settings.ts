import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import {
  DEFAULT_SETTINGS,
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
    keybindings: sanitizeKeybindings(obj.keybindings),
  };
};

type SettingsStore = Settings & {
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setLanguage: (language: AppLanguage) => void;
  setTheme: (theme: AppTheme) => void;
  setKeybinding: (action: keyof Keybindings, chord: string) => void;
  resetKeybindings: () => void;
  resetAll: () => void;
};

let storeHandle: LazyStore | null = null;
const getStore = (): LazyStore => {
  if (!storeHandle) storeHandle = new LazyStore(STORE_FILE);
  return storeHandle;
};

const persist = async (settings: {
  language: AppLanguage;
  theme: AppTheme;
  keybindings: Keybindings;
}): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(STORE_KEY, settings);
    await getStore().save();
  } catch (error) {
    console.warn("settings persist failed", error);
  }
};

const applyTheme = (theme: AppTheme): void => {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = theme;
};

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  ...DEFAULT_SETTINGS,
  hydrated: false,

  hydrate: async () => {
    if (!isTauri()) {
      applyTheme(DEFAULT_SETTINGS.theme);
      set({ hydrated: true });
      return;
    }
    try {
      const raw = await getStore().get<Settings>(STORE_KEY);
      const next = sanitizeSettings(raw);
      applyTheme(next.theme);
      set({ ...next, hydrated: true });
    } catch (error) {
      console.warn("settings hydrate failed", error);
      applyTheme(DEFAULT_SETTINGS.theme);
      set({ hydrated: true });
    }
  },

  setLanguage: (language) => {
    const next: Pick<Settings, "language" | "theme" | "keybindings"> = {
      language,
      theme: get().theme,
      keybindings: get().keybindings,
    };
    set({ language });
    void persist(next);
  },

  setTheme: (theme) => {
    applyTheme(theme);
    const next: Pick<Settings, "language" | "theme" | "keybindings"> = {
      language: get().language,
      theme,
      keybindings: get().keybindings,
    };
    set({ theme });
    void persist(next);
  },

  setKeybinding: (action, chord) => {
    const keybindings = { ...get().keybindings, [action]: chord };
    const next: Pick<Settings, "language" | "theme" | "keybindings"> = {
      language: get().language,
      theme: get().theme,
      keybindings,
    };
    set({ keybindings });
    void persist(next);
  },

  resetKeybindings: () => {
    const keybindings = DEFAULT_SETTINGS.keybindings;
    const next: Pick<Settings, "language" | "theme" | "keybindings"> = {
      language: get().language,
      theme: get().theme,
      keybindings,
    };
    set({ keybindings });
    void persist(next);
  },

  resetAll: () => {
    applyTheme(DEFAULT_SETTINGS.theme);
    set({ ...DEFAULT_SETTINGS, hydrated: true });
    void persist(DEFAULT_SETTINGS);
  },
}));

export const settingsSelector = {
  language: (s: SettingsStore): AppLanguage => s.language,
  theme: (s: SettingsStore): AppTheme => s.theme,
  keybindings: (s: SettingsStore): Keybindings => s.keybindings,
  hydrated: (s: SettingsStore): boolean => s.hydrated,
};
