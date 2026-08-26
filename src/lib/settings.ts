import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { invoke } from "@tauri-apps/api/core";
import {
  CHAT_BACKGROUND_FILENAME_PATTERN,
  COMMAND_LIST_MAX_ITEMS,
  COMMAND_LIST_MAX_LENGTH,
  DEFAULT_ANIMATIONS_ENABLED,
  DEFAULT_CHAT_BACKGROUND_IMAGE,
  DEFAULT_CHAT_BACKGROUND_OPACITY,
  DEFAULT_FONT_FAMILY,
  DEFAULT_FORCE_RESPONSE_LANGUAGE,
  DEFAULT_BUILD_AGENT_ENABLED,
  DEFAULT_LSP_ENABLED,
  DEFAULT_PLAN_AGENT_ENABLED,
  DEFAULT_MAX_WORKER_CORES,
  DEFAULT_NOTIFICATIONS_ENABLED,
  DEFAULT_REMINDER_INTERVAL,
  DEFAULT_RESPONSE_LANGUAGE,
  DEFAULT_SETTINGS,
  DEFAULT_TASK_COMPLETE_SOUND_ENABLED,
  DEFAULT_TEXT_SCALE,
  DEFAULT_TITLE_USE_FIRST_MESSAGE,
  DEFAULT_TRANSLUCENCY_ENABLED,
  DEFAULT_WINDOW_BOUNDS,
  DEFAULT_WORKSPACE_MEMORY_ENABLED,
  FONT_FAMILY_OPTIONS,
  MAX_REMINDER_INTERVAL,
  MAX_WORKER_CORES_AUTO,
  MIN_REMINDER_INTERVAL,
  MIN_WINDOW_HEIGHT,
  MIN_WINDOW_WIDTH,
  SUPPORTED_LANGUAGES,
  TEXT_SCALE_OPTIONS,
  hardwareThreadCount,
  type AppFontFamily,
  type AppLanguage,
  type AppTheme,
  type Keybindings,
  type Settings,
  type TextScale,
  type WindowBounds,
} from "@/types/settings";
import type { SelectedModel } from "@/types/chat";
import { isTauri } from "@/lib/platform";
import { syncWorkerCoreConfig } from "@/lib/worker-cores";

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

const FONT_FAMILY_MAX = 80;
const UNSAFE_FONT = /[;{}<>'"\\\n\r]/;

const sanitizeFontFamily = (value: unknown): AppFontFamily => {
  if (typeof value !== "string") return DEFAULT_FONT_FAMILY;
  const name = value.trim();
  if (name.length === 0 || name.length > FONT_FAMILY_MAX) return DEFAULT_FONT_FAMILY;
  if (UNSAFE_FONT.test(name)) return DEFAULT_FONT_FAMILY;
  return name;
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

const sanitizeChatBackgroundImage = (value: unknown): string | null => {
  if (typeof value !== "string") return DEFAULT_CHAT_BACKGROUND_IMAGE;
  const trimmed = value.trim();
  if (trimmed.length === 0) return DEFAULT_CHAT_BACKGROUND_IMAGE;
  if (!CHAT_BACKGROUND_FILENAME_PATTERN.test(trimmed)) return DEFAULT_CHAT_BACKGROUND_IMAGE;
  return trimmed;
};

const sanitizeChatBackgroundOpacity = (value: unknown): number => {
  const n = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
  if (!Number.isFinite(n)) return DEFAULT_CHAT_BACKGROUND_OPACITY;
  return Math.min(1, Math.max(0, n));
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

const sanitizeReminderInterval = (value: unknown): number => {
  const n = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
  if (!Number.isFinite(n)) return DEFAULT_REMINDER_INTERVAL;
  const rounded = Math.round(n);
  return Math.min(MAX_REMINDER_INTERVAL, Math.max(MIN_REMINDER_INTERVAL, rounded));
};

const sanitizeCommandList = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const entry of value) {
    if (typeof entry !== "string") continue;
    const trimmed = entry.trim();
    if (trimmed.length === 0) continue;
    const clipped =
      trimmed.length > COMMAND_LIST_MAX_LENGTH
        ? trimmed.slice(0, COMMAND_LIST_MAX_LENGTH)
        : trimmed;
    if (seen.has(clipped)) continue;
    seen.add(clipped);
    out.push(clipped);
    if (out.length >= COMMAND_LIST_MAX_ITEMS) break;
  }
  return out;
};

const sanitizeModelChoice = (value: unknown): SelectedModel | null => {
  if (value === null || value === undefined) return null;
  if (typeof value === "object") {
    const obj = value as Record<string, unknown>;
    if (
      typeof obj.providerId === "string" &&
      obj.providerId.length > 0 &&
      typeof obj.modelId === "string" &&
      obj.modelId.length > 0
    ) {
      return { providerId: obj.providerId, modelId: obj.modelId };
    }
    return null;
  }
  if (typeof value === "string" && value.length > 0) {
    const idx = value.indexOf("::");
    if (idx <= 0 || idx >= value.length - 2) return null;
    const providerId = value.slice(0, idx);
    const modelId = value.slice(idx + 2);
    return { providerId, modelId };
  }
  return null;
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
    fontFamily: sanitizeFontFamily(obj.fontFamily),
    maxWorkerCores: sanitizeMaxWorkerCores(obj.maxWorkerCores),
    reminderInterval: sanitizeReminderInterval(obj.reminderInterval),
    forceResponseLanguage: sanitizeBoolean(
      obj.forceResponseLanguage,
      DEFAULT_FORCE_RESPONSE_LANGUAGE,
    ),
    responseLanguage: sanitizeLanguage(obj.responseLanguage ?? DEFAULT_RESPONSE_LANGUAGE),
    blockedCommands: sanitizeCommandList(obj.blockedCommands),
    allowedCommands: sanitizeCommandList(obj.allowedCommands),
    notificationsEnabled: sanitizeBoolean(obj.notificationsEnabled, DEFAULT_NOTIFICATIONS_ENABLED),
    taskCompleteSoundEnabled: sanitizeBoolean(
      obj.taskCompleteSoundEnabled,
      DEFAULT_TASK_COMPLETE_SOUND_ENABLED,
    ),
    workspaceMemoryEnabled: sanitizeBoolean(
      obj.workspaceMemoryEnabled,
      DEFAULT_WORKSPACE_MEMORY_ENABLED,
    ),
    titleGenerationModel: sanitizeModelChoice(obj.titleGenerationModel),
    titleUseFirstMessage: sanitizeBoolean(
      obj.titleUseFirstMessage,
      DEFAULT_TITLE_USE_FIRST_MESSAGE,
    ),
    appGenerationModel: sanitizeModelChoice(obj.appGenerationModel),
    lspEnabled: sanitizeBoolean(obj.lspEnabled, DEFAULT_LSP_ENABLED),
    keybindings: sanitizeKeybindings(obj.keybindings),
    sessionSidebarOpen: sanitizeBoolean(
      obj.sessionSidebarOpen,
      DEFAULT_SETTINGS.sessionSidebarOpen,
    ),
    buildAgentEnabled: sanitizeBoolean(obj.buildAgentEnabled, DEFAULT_BUILD_AGENT_ENABLED),
    planAgentEnabled: sanitizeBoolean(obj.planAgentEnabled, DEFAULT_PLAN_AGENT_ENABLED),
    chatBackgroundImage: sanitizeChatBackgroundImage(obj.chatBackgroundImage),
    chatBackgroundOpacity: sanitizeChatBackgroundOpacity(obj.chatBackgroundOpacity),
  };
};

export type SettingsStore = Settings & {
  hydrated: boolean;
  chatBackgroundUrl: string | null;
  chatBackgroundLoading: boolean;
  hydrate: () => Promise<void>;
  setLanguage: (language: AppLanguage) => void;
  setTheme: (theme: AppTheme) => void;
  setMinimizeToTray: (enabled: boolean) => void;
  setTranslucencyEnabled: (enabled: boolean) => void;
  setAnimationsEnabled: (enabled: boolean) => void;
  setRememberWindowSize: (enabled: boolean) => void;
  setWindowBounds: (bounds: WindowBounds) => void;
  setTextScale: (scale: TextScale) => void;
  setFontFamily: (family: AppFontFamily) => void;
  setMaxWorkerCores: (cores: number) => void;
  setReminderInterval: (interval: number) => void;
  setForceResponseLanguage: (enabled: boolean) => void;
  setResponseLanguage: (language: AppLanguage) => void;
  setBlockedCommands: (commands: string[]) => void;
  setAllowedCommands: (commands: string[]) => void;
  setNotificationsEnabled: (enabled: boolean) => void;
  setTaskCompleteSoundEnabled: (enabled: boolean) => void;
  setWorkspaceMemoryEnabled: (enabled: boolean) => void;
  setTitleGenerationModel: (model: SelectedModel | null) => void;
  setTitleUseFirstMessage: (enabled: boolean) => void;
  setAppGenerationModel: (model: SelectedModel | null) => void;
  setLspEnabled: (enabled: boolean) => void;
  setKeybinding: (action: keyof Keybindings, chord: string) => void;
  setSessionSidebarOpen: (open: boolean) => void;
  setBuildAgentEnabled: (enabled: boolean) => void;
  setPlanAgentEnabled: (enabled: boolean) => void;
  setChatBackgroundImage: (filename: string | null, url: string | null) => void;
  setChatBackgroundOpacity: (opacity: number) => void;
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

const SYSTEM_SANS_STACK =
  '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

const applyFontFamily = (family: AppFontFamily): void => {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  if ((FONT_FAMILY_OPTIONS as readonly string[]).includes(family)) {
    root.dataset.font = family;
    root.style.removeProperty("--font-sans");
    return;
  }
  root.dataset.font = "custom";
  root.style.setProperty("--font-sans", `"${family}", ${SYSTEM_SANS_STACK}`);
};

const systemPrefersReducedMotion = (): boolean => {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
};

const applyChatBackground = (url: string | null, opacity: number): void => {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--chat-background-image", url ? `url("${url}")` : "none");
  root.style.setProperty("--chat-background-opacity", String(opacity));
  if (url) {
    root.dataset.chatBackground = "set";
  } else {
    delete root.dataset.chatBackground;
  }
};

const applyChrome = (settings: Settings, chatBackgroundUrl: string | null): void => {
  applyTheme(settings.theme);
  applyTranslucency(settings.translucencyEnabled);
  applyAnimations(settings.animationsEnabled && !systemPrefersReducedMotion());
  applyTextScale(settings.textScale);
  applyFontFamily(settings.fontFamily);
  applyChatBackground(chatBackgroundUrl, settings.chatBackgroundOpacity);
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
  fontFamily: state.fontFamily,
  maxWorkerCores: state.maxWorkerCores,
  reminderInterval: state.reminderInterval,
  forceResponseLanguage: state.forceResponseLanguage,
  responseLanguage: state.responseLanguage,
  blockedCommands: state.blockedCommands,
  allowedCommands: state.allowedCommands,
  notificationsEnabled: state.notificationsEnabled,
  taskCompleteSoundEnabled: state.taskCompleteSoundEnabled,
  workspaceMemoryEnabled: state.workspaceMemoryEnabled,
  titleGenerationModel: state.titleGenerationModel,
  titleUseFirstMessage: state.titleUseFirstMessage,
  appGenerationModel: state.appGenerationModel,
  lspEnabled: state.lspEnabled,
  keybindings: state.keybindings,
  sessionSidebarOpen: state.sessionSidebarOpen,
  buildAgentEnabled: state.buildAgentEnabled,
  planAgentEnabled: state.planAgentEnabled,
  chatBackgroundImage: state.chatBackgroundImage,
  chatBackgroundOpacity: state.chatBackgroundOpacity,
});

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  ...DEFAULT_SETTINGS,
  hydrated: false,
  chatBackgroundUrl: null,
  chatBackgroundLoading: false,

  hydrate: async () => {
    if (!isTauri()) {
      applyChrome(DEFAULT_SETTINGS, null);
      syncWorkerCoreConfig(DEFAULT_SETTINGS.maxWorkerCores);
      set({ hydrated: true });
      return;
    }
    try {
      const raw = await getStore().get<Settings>(STORE_KEY);
      const next = sanitizeSettings(raw);
      applyChrome(next, null);
      void syncMinimizeToTray(next.minimizeToTray);
      syncWorkerCoreConfig(next.maxWorkerCores);
      set({ ...next, hydrated: true });
      if (next.chatBackgroundImage) {
        set({ chatBackgroundLoading: true });
        invoke<string | null>("get_chat_background_data_url")
          .then((url) => {
            const current = get().chatBackgroundImage;
            if (current !== next.chatBackgroundImage) return;
            set({ chatBackgroundUrl: url });
            applyChrome(get(), url);
          })
          .catch((error: unknown) => {
            console.warn("chat background load failed", error);
            const current = get().chatBackgroundImage;
            if (current !== next.chatBackgroundImage) return;
            set({ chatBackgroundUrl: null });
            applyChrome(get(), null);
          })
          .finally(() => {
            const current = get().chatBackgroundImage;
            if (current === next.chatBackgroundImage) {
              set({ chatBackgroundLoading: false });
            }
          });
      }
    } catch (error) {
      console.warn("settings hydrate failed", error);
      applyChrome(DEFAULT_SETTINGS, null);
      syncWorkerCoreConfig(DEFAULT_SETTINGS.maxWorkerCores);
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

  setFontFamily: (family) => {
    const fontFamily = sanitizeFontFamily(family);
    applyFontFamily(fontFamily);
    set({ fontFamily });
    void persist(snapshot(get()));
  },

  setMaxWorkerCores: (cores) => {
    const maxWorkerCores = sanitizeMaxWorkerCores(cores);
    syncWorkerCoreConfig(maxWorkerCores);
    set({ maxWorkerCores });
    void persist(snapshot(get()));
  },

  setReminderInterval: (interval) => {
    set({ reminderInterval: sanitizeReminderInterval(interval) });
    void persist(snapshot(get()));
  },

  setForceResponseLanguage: (enabled) => {
    set({ forceResponseLanguage: enabled });
    void persist(snapshot(get()));
  },

  setResponseLanguage: (language) => {
    set({ responseLanguage: sanitizeLanguage(language) });
    void persist(snapshot(get()));
  },

  setBlockedCommands: (commands) => {
    set({ blockedCommands: sanitizeCommandList(commands) });
    void persist(snapshot(get()));
  },

  setAllowedCommands: (commands) => {
    set({ allowedCommands: sanitizeCommandList(commands) });
    void persist(snapshot(get()));
  },

  setNotificationsEnabled: (enabled) => {
    set({ notificationsEnabled: enabled });
    void persist(snapshot(get()));
  },

  setTaskCompleteSoundEnabled: (enabled) => {
    set({ taskCompleteSoundEnabled: enabled });
    void persist(snapshot(get()));
  },

  setWorkspaceMemoryEnabled: (enabled) => {
    set({ workspaceMemoryEnabled: enabled });
    void persist(snapshot(get()));
  },

  setTitleGenerationModel: (model) => {
    set({ titleGenerationModel: sanitizeModelChoice(model) });
    void persist(snapshot(get()));
  },

  setTitleUseFirstMessage: (enabled) => {
    set({ titleUseFirstMessage: enabled });
    void persist(snapshot(get()));
  },

  setAppGenerationModel: (model) => {
    set({ appGenerationModel: sanitizeModelChoice(model) });
    void persist(snapshot(get()));
  },

  setLspEnabled: (enabled) => {
    set({ lspEnabled: enabled });
    void persist(snapshot(get()));
  },

  setKeybinding: (action, chord) => {
    const keybindings = { ...get().keybindings, [action]: chord };
    set({ keybindings });
    void persist(snapshot(get()));
  },

  setSessionSidebarOpen: (open) => {
    set({ sessionSidebarOpen: open });
    void persist(snapshot(get()));
  },

  setBuildAgentEnabled: (enabled) => {
    set({ buildAgentEnabled: enabled });
    void persist(snapshot(get()));
  },

  setPlanAgentEnabled: (enabled) => {
    set({ planAgentEnabled: enabled });
    void persist(snapshot(get()));
  },

  setChatBackgroundImage: (filename, url) => {
    set({
      chatBackgroundImage: filename,
      chatBackgroundUrl: url,
      chatBackgroundLoading: false,
    });
    applyChrome(get(), url);
    void persist(snapshot(get()));
  },

  setChatBackgroundOpacity: (opacity) => {
    const next = sanitizeChatBackgroundOpacity(opacity);
    set({ chatBackgroundOpacity: next });
    applyChrome(get(), get().chatBackgroundUrl);
    void persist(snapshot(get()));
  },

  resetKeybindings: () => {
    set({ keybindings: DEFAULT_SETTINGS.keybindings });
    void persist(snapshot(get()));
  },

  resetAll: () => {
    applyChrome(DEFAULT_SETTINGS, null);
    void syncMinimizeToTray(DEFAULT_SETTINGS.minimizeToTray);
    if (isTauri()) {
      void invoke("clear_chat_background_image").catch((error: unknown) => {
        console.warn("chat background clear failed", error);
      });
    }
    set({
      ...DEFAULT_SETTINGS,
      hydrated: true,
      chatBackgroundUrl: null,
      chatBackgroundLoading: false,
    });
    void persist(DEFAULT_SETTINGS);
  },
}));
