import { create } from "zustand";
import { isTauri } from "@/lib/platform";
import { runListAgentsMdJob } from "@/lib/jobs";
import type { AgentsMdFile } from "@/types/agents-md";

const DESKTOP_REQUIRED = "Desktop shell required";

const toMessage = (error: unknown): string =>
  error instanceof Error ? error.message : typeof error === "string" ? error : "Unknown error";

type FetchPayload = {
  files: AgentsMdFile[];
  workspacePath: string | null;
};

type AgentsMdStore = {
  files: AgentsMdFile[];
  workspacePath: string | null;
  loading: boolean;
  error?: string;
  load: () => Promise<void>;
  refresh: () => Promise<{ error?: string; payload?: FetchPayload }>;
};

const fetchPayload = async (): Promise<FetchPayload> => {
  const bundle = await runListAgentsMdJob();
  return { files: bundle.contexts, workspacePath: bundle.workspacePath };
};

export const useAgentsMdStore = create<AgentsMdStore>((set) => ({
  files: [],
  workspacePath: null,
  loading: false,

  load: async () => {
    set({ loading: true, error: undefined });
    if (!isTauri()) {
      set({ files: [], workspacePath: null, loading: false });
      return;
    }
    try {
      const payload = await fetchPayload();
      set({ ...payload, loading: false });
    } catch (error) {
      set({ loading: false, error: toMessage(error) });
    }
  },

  refresh: async () => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      const payload = await fetchPayload();
      set({ ...payload, error: undefined });
      return { payload };
    } catch (error) {
      const message = toMessage(error);
      set({ error: message });
      return { error: message };
    }
  },
}));
