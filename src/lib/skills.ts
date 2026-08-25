import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";
import type { SkillContext, SkillMeta } from "@/types/skills";

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
  load: () => Promise<void>;
  refresh: () => Promise<{ error?: string; payload?: FetchPayload }>;
  setWorkspacePath: (path: string) => Promise<{ error?: string }>;
  readMeta: (rootPath: string, name: string) => Promise<{ error?: string; meta?: SkillMeta }>;
  readFile: (path: string) => Promise<{ error?: string; content?: string }>;
  create: (rootPath: string, name: string, description: string) => Promise<{ error?: string }>;
  updateContent: (path: string, content: string) => Promise<{ error?: string }>;
  remove: (rootPath: string, name: string) => Promise<{ error?: string }>;
};

const fetchPayload = async (): Promise<FetchPayload> => {
  const [contexts, workspacePath] = await Promise.all([
    invoke<SkillContext[]>("list_skills"),
    invoke<string | null>("get_workspace_path"),
  ]);
  return { contexts, workspacePath };
};

export const useSkillsStore = create<SkillsStore>((set, get) => ({
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

  setWorkspacePath: async (path) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke("set_workspace_path", { path });
      const payload = await fetchPayload();
      set({ ...payload, error: undefined });
      return {};
    } catch (error) {
      const message = toMessage(error);
      set({ error: message });
      return { error: message };
    }
  },

  readMeta: async (rootPath, name) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      const meta = await invoke<SkillMeta>("read_skill_meta", {
        input: { rootPath, name },
      });
      return { meta };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  readFile: async (path) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      const content = await invoke<string>("read_skill_file", {
        input: { path },
      });
      return { content };
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  create: async (rootPath, name, description) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke("create_skill", { input: { rootPath, name, description } });
      await get()
        .refresh()
        .catch(() => undefined);
      return {};
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  updateContent: async (path, content) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke("update_skill_content", { input: { path, content } });
      return {};
    } catch (error) {
      return { error: toMessage(error) };
    }
  },

  remove: async (rootPath, name) => {
    if (!isTauri()) return { error: DESKTOP_REQUIRED };
    try {
      await invoke("delete_skill", { input: { rootPath, name } });
      return {};
    } catch (error) {
      return { error: toMessage(error) };
    }
  },
}));
