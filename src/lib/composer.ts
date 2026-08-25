import { create } from "zustand";

type ComposerStore = {
  value: string;
  setValue: (next: string) => void;
  clear: () => void;
};

export const useComposerStore = create<ComposerStore>((set) => ({
  value: "",
  setValue: (next) => set({ value: next }),
  clear: () => set({ value: "" }),
}));
