import { create } from "zustand";

type ShellOutputState = {
  open: boolean;
  messageId: string | null;
  sessionId: string | null;
  command: string;
  openFor: (messageId: string, command: string, sessionId: string) => void;
  close: () => void;
};

export const useShellOutputStore = create<ShellOutputState>((set) => ({
  open: false,
  messageId: null,
  sessionId: null,
  command: "",
  openFor: (messageId, command, sessionId) => set({ open: true, messageId, command, sessionId }),
  close: () => set({ open: false, messageId: null, sessionId: null, command: "" }),
}));
