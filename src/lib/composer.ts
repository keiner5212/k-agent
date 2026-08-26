import { create } from "zustand";
import { LazyStore } from "@tauri-apps/plugin-store";
import { MAX_CHAT_ATTACHMENTS } from "@/lib/attachments";
import { isTauri } from "@/lib/platform";
import type { ChatAttachment } from "@/types/chat";

const STORE_FILE = "settings.json";
const KEY_SELECTED_AGENT = "selectedAgent";

let storeHandle: LazyStore | null = null;
const getStore = (): LazyStore => {
  if (!storeHandle) storeHandle = new LazyStore(STORE_FILE);
  return storeHandle;
};

const persistSelectedAgent = async (agent: string): Promise<void> => {
  if (!isTauri()) return;
  try {
    await getStore().set(KEY_SELECTED_AGENT, agent);
    await getStore().save();
  } catch (error) {
    console.warn("selected agent persist failed", error);
  }
};

export type ComposerMode = "prompt" | "shell";

export type ComposerStore = {
  value: string;
  selectedAgent: string;
  agentHydrated: boolean;
  mode: ComposerMode;
  attachments: ChatAttachment[];
  setValue: (next: string) => void;
  setSelectedAgent: (next: string) => void;
  hydrateAgent: () => Promise<void>;
  setMode: (next: ComposerMode) => void;
  toggleMode: () => void;
  addAttachments: (items: ChatAttachment[]) => void;
  removeAttachment: (id: string) => void;
  setAttachments: (items: ChatAttachment[]) => void;
  clear: () => void;
};

export const useComposerStore = create<ComposerStore>((set) => ({
  value: "",
  selectedAgent: "",
  agentHydrated: false,
  mode: "prompt",
  attachments: [],
  setValue: (next) => set({ value: next }),
  setSelectedAgent: (next) => {
    set({ selectedAgent: next });
    void persistSelectedAgent(next);
  },
  hydrateAgent: async () => {
    if (!isTauri()) {
      set({ agentHydrated: true });
      return;
    }
    try {
      const stored = await getStore().get<string>(KEY_SELECTED_AGENT);
      if (typeof stored === "string") {
        set({ selectedAgent: stored, agentHydrated: true });
      } else {
        set({ agentHydrated: true });
      }
    } catch (error) {
      console.warn("selected agent hydrate failed", error);
      set({ agentHydrated: true });
    }
  },
  setMode: (next) => set({ mode: next }),
  toggleMode: () => set((state) => ({ mode: state.mode === "prompt" ? "shell" : "prompt" })),
  addAttachments: (items) =>
    set((state) => ({
      attachments: [...state.attachments, ...items].slice(0, MAX_CHAT_ATTACHMENTS),
    })),
  removeAttachment: (id) =>
    set((state) => ({ attachments: state.attachments.filter((item) => item.id !== id) })),
  setAttachments: (items) => set({ attachments: items }),
  clear: () => set({ value: "", attachments: [] }),
}));
