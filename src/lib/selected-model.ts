import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { isTauri } from "@/lib/platform";
import type { SelectedModel } from "@/types/chat";

const STORE_FILE = "settings.json";
const KEY_SELECTED_MODEL = "selectedModel";

let storeHandle: LazyStore | null = null;
const getStore = (): LazyStore => {
  if (!storeHandle) storeHandle = new LazyStore(STORE_FILE);
  return storeHandle;
};

type SelectionState = {
  selection: SelectedModel | null;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  select: (next: SelectedModel | null) => void;
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

export const useSelectionStore = create<SelectionState>((set) => ({
  selection: null,
  hydrated: false,

  hydrate: async () => {
    if (!isTauri()) {
      set({ hydrated: true });
      return;
    }
    try {
      const selection = (await getStore().get<SelectedModel | null>(KEY_SELECTED_MODEL)) ?? null;
      set({ selection, hydrated: true });
    } catch (error) {
      console.warn("selection hydrate failed", error);
      set({ hydrated: true });
    }
  },

  select: (next) => {
    set({ selection: next });
    void persistSelection(next);
  },
}));
