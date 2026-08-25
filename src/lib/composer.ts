import { create } from "zustand";

type ComposerStore = {
  value: string;
  selectedAgent: string;
  setValue: (next: string) => void;
  setSelectedAgent: (next: string) => void;
  clear: () => void;
};

export const useComposerStore = create<ComposerStore>((set) => ({
  value: "",
  selectedAgent: "",
  setValue: (next) => set({ value: next }),
  setSelectedAgent: (next) => set({ selectedAgent: next }),
  clear: () => set({ value: "" }),
}));
