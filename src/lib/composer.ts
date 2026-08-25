import { create } from "zustand";

export type ComposerMode = "prompt" | "shell";

export type ComposerStore = {
  value: string;
  selectedAgent: string;
  mode: ComposerMode;
  setValue: (next: string) => void;
  setSelectedAgent: (next: string) => void;
  setMode: (next: ComposerMode) => void;
  toggleMode: () => void;
  clear: () => void;
};

export const useComposerStore = create<ComposerStore>((set) => ({
  value: "",
  selectedAgent: "",
  mode: "prompt",
  setValue: (next) => set({ value: next }),
  setSelectedAgent: (next) => set({ selectedAgent: next }),
  setMode: (next) => set({ mode: next }),
  toggleMode: () => set((state) => ({ mode: state.mode === "prompt" ? "shell" : "prompt" })),
  clear: () => set({ value: "" }),
}));
