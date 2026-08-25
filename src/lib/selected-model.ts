import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { isTauri } from "@/lib/platform";
import type { SelectedModel } from "@/types/chat";

const STORE_FILE = "settings.json";
const KEY_SELECTED_MODEL = "selectedModel";
const KEY_EFFORT_PREFIX = "modelEffort:";

let storeHandle: LazyStore | null = null;
const getStore = (): LazyStore => {
  if (!storeHandle) storeHandle = new LazyStore(STORE_FILE);
  return storeHandle;
};

const effortKey = (selection: SelectedModel): string =>
  `${KEY_EFFORT_PREFIX}${selection.providerId}:${selection.modelId}`;

type SelectionState = {
  selection: SelectedModel | null;
  effortByModel: Record<string, string>;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  select: (next: SelectedModel | null) => void;
  setEffort: (effort: string | null) => void;
};

const persistSelection = async (selection: SelectedModel | null): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(KEY_SELECTED_MODEL, selection);
    await getStore().save();
  } catch (error) {
    console.warn("selected model persist failed", error);
  }
};

const persistEffort = async (selection: SelectedModel, effort: string): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(effortKey(selection), effort);
    await getStore().save();
  } catch (error) {
    console.warn("model effort persist failed", error);
  }
};

export const useSelectionStore = create<SelectionState>((set, get) => ({
  selection: null,
  effortByModel: {},
  hydrated: false,

  hydrate: async () => {
    if (!isTauri()) {
      set({ hydrated: true });
      return;
    }
    try {
      const store = getStore();
      const selection = (await store.get<SelectedModel | null>(KEY_SELECTED_MODEL)) ?? null;
      const effortByModel: Record<string, string> = {};
      if (selection) {
        const storedEffort = await store.get<string>(effortKey(selection));
        if (storedEffort) {
          effortByModel[effortKey(selection)] = storedEffort;
        }
      }
      set({ selection, effortByModel, hydrated: true });
    } catch (error) {
      console.warn("selection hydrate failed", error);
      set({ hydrated: true });
    }
  },

  select: (next) => {
    set({ selection: next });
    void persistSelection(next);
  },

  setEffort: (effort) => {
    const selection = get().selection;
    if (!selection) return;
    const key = effortKey(selection);
    set((state) => {
      const next = { ...state.effortByModel };
      if (effort === null) {
        delete next[key];
      } else {
        next[key] = effort;
      }
      return { effortByModel: next };
    });
    if (effort !== null) void persistEffort(selection, effort);
  },
}));

export const selectEffort = (state: SelectionState): string | null => {
  const selection = state.selection;
  if (!selection) return null;
  return state.effortByModel[effortKey(selection)] ?? null;
};
