import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";
import { runListAgentsJob } from "@/lib/jobs";
import type { AgentContext, AgentContextKind, AgentMeta, AgentSkillRef } from "@/types/agents";

const DESKTOP_REQUIRED = "Desktop shell required";

const toMessage = (error: unknown): string =>
  error instanceof Error ? error.message : typeof error === "string" ? error : "Unknown error";

type FetchPayload = {
  contexts: AgentContext[];
  workspacePath: string | null;
};

export type AgentWriteInput = {
  name: string;
  description: string;
  personality: string;
  skills: AgentSkillRef[];
  tools: string[];
};

type AgentsStore = {
  contexts: AgentContext[];
  workspacePath: string | null;
  loading: boolean;
  error?: string;
  load: () => Promise<void>;
  refresh: () => Promise<{ error?: string; payload?: FetchPayload }>;
  create: (
    rootPath: string,
    kind: AgentContextKind,
    input: AgentWriteInput,
  ) => Promise<{ error?: string }>;
  update: (
    path: string,
    kind: AgentContextKind,
    input: AgentWriteInput,
  ) => Promise<{ error?: string }>;
  remove: (rootPath: string, name: string) => Promise<{ error?: string }>;
};

const fetchPayload = async (): Promise<FetchPayload> => runListAgentsJob();

export const useAgentsStore = create<AgentsStore>((set, get) => ({
  contexts: [],
  workspacePath: null,
  loading: false,

  load: async () => {
    set({ loading: true, error: undefined });
    if (!isTauri()) {
      set({ contexts: [], workspacePath: null, loading: false });
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

  create: async (rootPath, kind, input) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke<AgentMeta>("create_agent", {
        input: { rootPath, kind, ...input },
      });
      await get()
        .refresh()
        .catch(() => undefined);
      return {};
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  update: async (path, kind, input) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke<AgentMeta>("update_agent", {
        input: { path, kind, ...input },
      });
      await get()
        .refresh()
        .catch(() => undefined);
      return {};
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  remove: async (rootPath, name) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke("delete_agent", { input: { rootPath, name } });
      return {};
    } catch (error) {
      return { error: toMessage(error) };
    }
  },
}));
