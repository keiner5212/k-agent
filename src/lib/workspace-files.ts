import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { dirsToLoadForMention, ROOT_DIR } from "@/lib/file-mentions";
import { runListWorkspaceDirJob } from "@/lib/jobs";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { perfLog } from "@/lib/perf-log";
import { acquireWorkerCores } from "@/lib/worker-cores";
import type { WorkspaceEntry } from "@/types/workspace-files";

export const DIR_CACHE_TTL_MS = 1000;

export type CachedDir = {
  entries: WorkspaceEntry[];
  loadedAt: number;
};

type WorkspaceFilesStore = {
  dirs: Record<string, CachedDir>;
  workspacePath: string | null;
  loadingDirs: string[];
  error?: string;
  hasDir: (path: string) => boolean;
  ensureDirLoaded: (relativeDir: string) => Promise<void>;
  ensureMentionScope: (query: string) => Promise<void>;
  prefetchDir: (relativeDir: string) => void;
  ensureRootLoaded: () => Promise<void>;
  invalidate: () => void;
};

const dirLoads = new Map<string, Promise<void>>();

const normalizeDir = (relativeDir: string): string =>
  relativeDir.trim().replace(/\\/g, "/").replace(/\/+$/, "");

const isFresh = (cached: CachedDir | undefined, now: number): boolean =>
  cached !== undefined && now - cached.loadedAt < DIR_CACHE_TTL_MS;

export const hasDirInCache = (dirs: Record<string, CachedDir>, path: string): boolean => {
  const normalized = path.trim().replace(/\\/g, "/").replace(/\/+$/, "");
  if (!normalized) return true;
  if (dirs[normalized]) return true;
  const slash = normalized.lastIndexOf("/");
  const parent = slash === -1 ? ROOT_DIR : normalized.slice(0, slash);
  const siblings = dirs[parent]?.entries;
  return Boolean(siblings?.some((entry) => entry.kind === "dir" && entry.path === normalized));
};

export const useWorkspaceFilesStore = create<WorkspaceFilesStore>((set, get) => ({
  dirs: {},
  workspacePath: null,
  loadingDirs: [],
  error: undefined,

  hasDir: (path) => hasDirInCache(get().dirs, path),

  ensureDirLoaded: async (relativeDir) => {
    if (!isTauri()) return;
    const normalized = normalizeDir(relativeDir);
    const cached = get().dirs[normalized];
    const now = performance.now();
    if (isFresh(cached, now)) return;

    const inflight = dirLoads.get(normalized);
    if (inflight) {
      if (cached) return;
      await inflight;
      return;
    }

    const loadPromise = (async () => {
      const start = performance.now();
      const hadCache = Boolean(get().dirs[normalized]);
      if (!hadCache) {
        set((current) => ({
          loadingDirs: current.loadingDirs.includes(normalized)
            ? current.loadingDirs
            : [...current.loadingDirs, normalized],
          error: undefined,
        }));
      }
      const lease = acquireWorkerCores("listWorkspaceDir", 1);
      try {
        const workspacePath = await invoke<string | null>("get_workspace_path");
        if (!workspacePath) {
          set({ dirs: {}, workspacePath: null, loadingDirs: [], error: undefined });
          return;
        }
        const entries = await runListWorkspaceDirJob(normalized);
        set((current) => ({
          dirs: {
            ...current.dirs,
            [normalized]: { entries, loadedAt: performance.now() },
          },
          workspacePath,
          loadingDirs: current.loadingDirs.filter((dir) => dir !== normalized),
          error: undefined,
        }));
        perfLog("workspaceFiles.loadDir", performance.now() - start, {
          dir: normalized || ".",
          count: entries.length,
          cores: lease.cores,
          swr: hadCache,
        });
      } catch (error) {
        set((current) => ({
          loadingDirs: current.loadingDirs.filter((dir) => dir !== normalized),
          error: current.dirs[normalized] ? current.error : ipcErrorMessage(error),
        }));
        perfLog("workspaceFiles.loadDir.error", performance.now() - start, {
          dir: normalized || ".",
        });
      } finally {
        lease.release();
      }
    })();

    dirLoads.set(normalized, loadPromise);
    if (!cached) {
      try {
        await loadPromise;
      } finally {
        dirLoads.delete(normalized);
      }
      return;
    }
    void loadPromise.finally(() => {
      dirLoads.delete(normalized);
    });
  },

  ensureMentionScope: async (query) => {
    const start = performance.now();
    const dirs = dirsToLoadForMention(query, get().hasDir);
    await Promise.all(dirs.map((dir) => get().ensureDirLoaded(dir)));
    const elapsed = performance.now() - start;
    if (elapsed >= 8) {
      perfLog("workspaceFiles.ensureMentionScope", elapsed, {
        query,
        dirs: dirs.length,
      });
    }
  },

  prefetchDir: (relativeDir) => {
    void get().ensureDirLoaded(relativeDir);
  },

  ensureRootLoaded: async () => {
    await get().ensureDirLoaded(ROOT_DIR);
  },

  invalidate: () => {
    dirLoads.clear();
    set({
      dirs: {},
      workspacePath: null,
      loadingDirs: [],
      error: undefined,
    });
  },
}));

export const workspaceFilesLoading = (
  state: WorkspaceFilesStore,
  relativeDir = ROOT_DIR,
): boolean => {
  const dir = normalizeDir(relativeDir);
  return state.loadingDirs.includes(dir) && !state.dirs[dir];
};
