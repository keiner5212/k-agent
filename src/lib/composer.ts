import { create } from "zustand";
import { DEFAULT_AGENT_MODE, isAgentMode, type AgentMode } from "@/types/chat";

type ComposerStore = {
  value: string;
  agentMode: AgentMode;
  setValue: (next: string) => void;
  setAgentMode: (next: string) => void;
  clear: () => void;
};

export const useComposerStore = create<ComposerStore>((set) => ({
  value: "",
  agentMode: DEFAULT_AGENT_MODE,
  setValue: (next) => set({ value: next }),
  setAgentMode: (next) => {
    if (!isAgentMode(next)) return;
    set({ agentMode: next });
  },
  clear: () => set({ value: "" }),
}));
