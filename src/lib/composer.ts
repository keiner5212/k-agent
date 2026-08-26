import { create } from "zustand";
import { MAX_CHAT_ATTACHMENTS } from "@/lib/attachments";
import type { ChatAttachment } from "@/types/chat";

export type ComposerMode = "prompt" | "shell";

export type ComposerStore = {
  value: string;
  selectedAgent: string;
  mode: ComposerMode;
  attachments: ChatAttachment[];
  setValue: (next: string) => void;
  setSelectedAgent: (next: string) => void;
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
  mode: "prompt",
  attachments: [],
  setValue: (next) => set({ value: next }),
  setSelectedAgent: (next) => set({ selectedAgent: next }),
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
