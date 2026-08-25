import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { dirsToLoadForMention } from "@/lib/file-mentions";
import { runListWorkspaceDirJob } from "@/lib/jobs";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { perfLog } from "@/lib/perf-log";
import type { WorkspaceEntry } from "@/types/workspace-files";

const ROOT_DIR = "";

type WorkspaceFilesStore = {
  entries: WorkspaceEntry[];
  workspacePath: string | null;
  loadedDirs: string[];
  loadingDirs: string[];
  error?: string;
  ensureDirLoaded: (relativeDir: string) => Promise<void>;
  ensureMentionScope: (query: string) => Promise<void>;
  ensureRootLoaded: () => Promise<void>;
  invalidate: () => void;
};

const entryKey = (entry: WorkspaceEntry): string => `${entry.kind}:${entry.path}`;

const mergeEntries = (
  current: readonly WorkspaceEntry[],
  next: readonly WorkspaceEntry[],
): WorkspaceEntry[] => {
  const map = new Map(current.map((entry) => [entryKey(entry), entry]));
  for (const entry of next) {
    map.set(entryKey(entry), entry);
  }
  return [...map.values()];
};

const dirLoads = new Map<string, Promise<void>>();

export const useWorkspaceFilesStore = create<WorkspaceFilesStore>((set, get) => ({
  entries: [],
  workspacePath: null,
  loadedDirs: [],
  loadingDirs: [],
  error: undefined,

  ensureDirLoaded: async (relativeDir) => {
    if (!isTauri()) return;
    const normalized = relativeDir.trim().replace(/\\/g, "/");
    const state = get();
    if (state.loadedDirs.includes(normalized)) return;

    const inflight = dirLoads.get(normalized);
    if (inflight) {
      await inflight;
      return;
    }

    const loadPromise = (async () => {
      const start = performance.now();
      set((current) => ({
        loadingDirs: current.loadingDirs.includes(normalized)
          ? current.loadingDirs
          : [...current.loadingDirs, normalized],
        error: undefined,
      }));
      try {
        const workspacePath = await invoke<string | null>("get_workspace_path");
        if (!workspacePath) {
          set({ entries: [], workspacePath: null, loadedDirs: [], loadingDirs: [] });
          return;
        }
        const entries = await runListWorkspaceDirJob(normalized);
        set((current) => ({
          entries: mergeEntries(current.entries, entries),
          workspacePath,
          loadedDirs: current.loadedDirs.includes(normalized)
            ? current.loadedDirs
            : [...current.loadedDirs, normalized],
          loadingDirs: current.loadingDirs.filter((dir) => dir !== normalized),
          error: undefined,
        }));
        perfLog("workspaceFiles.loadDir", performance.now() - start, {
          dir: normalized || ".",
          count: entries.length,
        });
      } catch (error) {
        set((current) => ({
          loadingDirs: current.loadingDirs.filter((dir) => dir !== normalized),
          error: ipcErrorMessage(error),
        }));
        perfLog("workspaceFiles.loadDir.error", performance.now() - start, {
          dir: normalized || ".",
        });
      }
    })();

    dirLoads.set(normalized, loadPromise);
    try {
      await loadPromise;
    } finally {
      dirLoads.delete(normalized);
    }
  },

  ensureMentionScope: async (query) => {
    const start = performance.now();
    const dirs = dirsToLoadForMention(query, get().entries);
    await Promise.all(dirs.map((dir) => get().ensureDirLoaded(dir)));
    perfLog("workspaceFiles.ensureMentionScope", performance.now() - start, {
      query,
      dirs: dirs.length,
    });
  },

  ensureRootLoaded: async () => {
    await get().ensureDirLoaded(ROOT_DIR);
  },

  invalidate: () => {
    dirLoads.clear();
    set({
      entries: [],
      workspacePath: null,
      loadedDirs: [],
      loadingDirs: [],
      error: undefined,
    });
  },
}));

export const workspaceFilesLoading = (state: WorkspaceFilesStore): boolean =>
  state.loadingDirs.length > 0;
