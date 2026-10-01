import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { DESKTOP_REQUIRED, ipcErrorMessage, isTauri } from "@/lib/platform";
import { runListAgentsMdJob } from "@/lib/jobs";
import type { AgentsMdFile, AgentsMdKind } from "@/types/agents-md";

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
  write: (kind: AgentsMdKind, content: string) => Promise<{ error?: string }>;
  remove: (kind: AgentsMdKind) => Promise<{ error?: string }>;
};

const fetchPayload = async (): Promise<FetchPayload> => {
  const bundle = await runListAgentsMdJob();
  return { files: bundle.contexts, workspacePath: bundle.workspacePath };
};

export const useAgentsMdStore = create<AgentsMdStore>((set, get) => ({
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
      set({ loading: false, error: ipcErrorMessage(error) });
    }
  },

  refresh: async () => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      const payload = await fetchPayload();
      set({ ...payload, error: undefined });
      return { payload };
    } catch (error) {
      const message = ipcErrorMessage(error);
      set({ error: message });
      return { error: message };
    }
  },

  write: async (kind, content) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke("write_agents_md", { input: { kind, content } });
      await get().refresh();
      return {};
    } catch (error) {
      return { error: ipcErrorMessage(error) };
    }
  },

  remove: async (kind) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke("delete_agents_md", { input: { kind } });
      await get().refresh();
      return {};
    } catch (error) {
      return { error: ipcErrorMessage(error) };
    }
  },
}));
