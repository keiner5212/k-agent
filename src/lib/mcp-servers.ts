import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { DESKTOP_REQUIRED, ipcErrorMessage, isTauri } from "@/lib/platform";
import type { McpServer, McpServerDraft } from "@/types/mcp-servers";

export type McpServerMutationResult = {
  server?: McpServer;
  error?: string;
};

const replaceServer = (servers: McpServer[], server: McpServer): McpServer[] => {
  const idx = servers.findIndex((item) => item.id === server.id);
  if (idx < 0) return [...servers, server];
  const next = [...servers];
  next[idx] = server;
  return next;
};

const runMutation = async (
  work: () => Promise<McpServer | undefined>,
): Promise<McpServerMutationResult> => {
  if (!isTauri()) return { error: DESKTOP_REQUIRED };
  try {
    const server = await work();
    return server ? { server } : {};
  } catch (error) {
    return { error: ipcErrorMessage(error) };
  }
};

type McpServersStore = {
  servers: McpServer[];
  loading: boolean;
  error?: string;
  load: () => Promise<void>;
  save: (draft: McpServerDraft) => Promise<McpServerMutationResult>;
  refreshTools: (id: string) => Promise<McpServerMutationResult>;
  setEnabled: (id: string, enabled: boolean) => Promise<McpServerMutationResult>;
  remove: (id: string) => Promise<McpServerMutationResult>;
};

export const useMcpServersStore = create<McpServersStore>((set) => ({
  servers: [],
  loading: false,

  load: async () => {
    set({ loading: true, error: undefined });
    if (!isTauri()) {
      set({ servers: [], loading: false });
      return;
    }
    try {
      const servers = await invoke<McpServer[]>("list_mcp_servers");
      set({ servers, loading: false });
    } catch (error) {
      set({ loading: false, error: ipcErrorMessage(error) });
    }
  },

  save: async (draft) => {
    const result = await runMutation(async () => {
      const server = await invoke<McpServer>("save_mcp_server", {
        input: {
          id: draft.id ?? null,
          name: draft.name,
          enabled: draft.enabled,
          transport: draft.transport,
          command: draft.command ?? null,
          args: draft.args ?? [],
          cwd: draft.cwd ?? null,
          url: draft.url ?? null,
          env: draft.env ?? {},
          headers: draft.headers ?? {},
          clearSecrets: Boolean(draft.clearSecrets),
        },
      });
      set((state) => ({ servers: replaceServer(state.servers, server), error: undefined }));
      return server;
    });
    if (result.error) set({ error: result.error });
    return result;
  },

  refreshTools: async (id) => {
    const result = await runMutation(async () => {
      const server = await invoke<McpServer>("refresh_mcp_server_tools", { id });
      set((state) => ({ servers: replaceServer(state.servers, server), error: undefined }));
      return server;
    });
    if (result.error) set({ error: result.error });
    return result;
  },

  setEnabled: async (id, enabled) => {
    const result = await runMutation(async () => {
      const server = await invoke<McpServer>("set_mcp_server_enabled", { id, enabled });
      set((state) => ({ servers: replaceServer(state.servers, server), error: undefined }));
      return server;
    });
    if (result.error) set({ error: result.error });
    return result;
  },

  remove: async (id) => {
    const result = await runMutation(async () => {
      await invoke("delete_mcp_server", { id });
      set((state) => ({
        servers: state.servers.filter((item) => item.id !== id),
        error: undefined,
      }));
      return undefined;
    });
    if (result.error) set({ error: result.error });
    return result;
  },
}));
