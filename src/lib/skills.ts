import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";
import type { SkillContext } from "@/types/skills";

const DESKTOP_REQUIRED = "Desktop shell required";

const toMessage = (error: unknown): string =>
  error instanceof Error ? error.message : typeof error === "string" ? error : "Unknown error";

type FetchPayload = {
  contexts: SkillContext[];
  workspacePath: string | null;
};

type SkillsStore = {
  contexts: SkillContext[];
  workspacePath: string | null;
  loading: boolean;
  error?: string;
  load: (globalPath: string) => Promise<void>;
  refresh: (globalPath: string) => Promise<{ error?: string; payload?: FetchPayload }>;
};

const fetchPayload = async (globalPath: string): Promise<FetchPayload> => {
  const [contexts, workspacePath] = await Promise.all([
    invoke<SkillContext[]>("list_skills", { input: { globalPath } }),
    invoke<string | null>("get_workspace_path"),
  ]);
  return { contexts, workspacePath };
};

export const useSkillsStore = create<SkillsStore>((set) => ({
  contexts: [],
  workspacePath: null,
  loading: false,

  load: async (globalPath) => {
    set({ loading: true, error: undefined });
    if (!isTauri()) {
      set({ contexts: [], workspacePath: null, loading: false });
      return;
    }
    try {
      const payload = await fetchPayload(globalPath);
      set({ ...payload, loading: false });
    } catch (error) {
      set({ loading: false, error: toMessage(error) });
    }
  },

  refresh: async (globalPath) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      const payload = await fetchPayload(globalPath);
      set({ ...payload, error: undefined });
      return { payload };
    } catch (error) {
      const message = toMessage(error);
      set({ error: message });
      return { error: message };
    }
  },
}));