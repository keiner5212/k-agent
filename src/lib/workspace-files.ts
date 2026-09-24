import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { dirsToLoadForMention } from "@/lib/file-mentions";
import { runListWorkspaceDirJob, runSearchWorkspaceFilesJob } from "@/lib/jobs";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { perfLog } from "@/lib/perf-log";
import { acquireWorkerCores } from "@/lib/worker-cores";
import type { WorkspaceEntry } from "@/types/workspace-files";

export const DIR_CACHE_TTL_MS = 1000;
const WORKSPACE_ROOT_DIR = "";

export type CachedDir = {
  entries: WorkspaceEntry[];
  loadedAt: number;
};

export type WorkspaceFilesStore = {
  dirs: Record<string, CachedDir>;
  workspacePath: string | null;
  loadingDirs: string[];
  mentionQuery: string;
  mentionHits: WorkspaceEntry[];
  error?: string;
  hasDir: (path: string) => boolean;
  ensureDirLoaded: (relativeDir: string) => Promise<void>;
  ensureMentionScope: (query: string) => Promise<void>;
  searchMention: (query: string) => Promise<void>;
  prefetchDir: (relativeDir: string) => void;
  ensureRootLoaded: () => Promise<void>;
  invalidate: () => void;
};

const dirLoads = new Map<string, Promise<void>>();
let mentionSearchGen = 0;

const normalizeDir = (relativeDir: string): string =>
  relativeDir.trim().replace(/\\/g, "/").replace(/\/+$/, "");

const isFresh = (cached: CachedDir | undefined, now: number): boolean =>
  cached !== undefined && now - cached.loadedAt < DIR_CACHE_TTL_MS;

export const hasDirInCache = (dirs: Record<string, CachedDir>, path: string): boolean => {
  const normalized = path.trim().replace(/\\/g, "/").replace(/\/+$/, "");
  if (!normalized) return true;
  if (dirs[normalized]) return true;
  const slash = normalized.lastIndexOf("/");
  const parent = slash === -1 ? WORKSPACE_ROOT_DIR : normalized.slice(0, slash);
  const siblings = dirs[parent]?.entries;
  return Boolean(siblings?.some((entry) => entry.kind === "dir" && entry.path === normalized));
};

export const useWorkspaceFilesStore = create<WorkspaceFilesStore>((set, get) => ({
  dirs: {},
  workspacePath: null,
  loadingDirs: [],
  mentionQuery: "",
  mentionHits: [],
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

  searchMention: async (query) => {
    if (!isTauri()) return;
    const normalized = query.trim();
    if (!normalized) {
      set({ mentionQuery: "", mentionHits: [] });
      return;
    }
    const gen = mentionSearchGen + 1;
    mentionSearchGen = gen;
    const start = performance.now();
    const lease = acquireWorkerCores("searchWorkspaceFiles", 1);
    try {
      const entries = await runSearchWorkspaceFilesJob(normalized);
      if (gen !== mentionSearchGen) return;
      set({ mentionQuery: normalized, mentionHits: entries });
      perfLog("workspaceFiles.searchMention", performance.now() - start, {
        query: normalized,
        count: entries.length,
        cores: lease.cores,
      });
    } catch (error) {
      if (gen !== mentionSearchGen) return;
      set({ mentionQuery: normalized, mentionHits: [] });
      perfLog("workspaceFiles.searchMention.error", performance.now() - start, {
        query: normalized,
        error: ipcErrorMessage(error),
      });
    } finally {
      lease.release();
    }
  },

  prefetchDir: (relativeDir) => {
    void get().ensureDirLoaded(relativeDir);
  },

  ensureRootLoaded: async () => {
    await get().ensureDirLoaded(WORKSPACE_ROOT_DIR);
  },

  invalidate: () => {
    dirLoads.clear();
    mentionSearchGen += 1;
    set({
      dirs: {},
      workspacePath: null,
      loadingDirs: [],
      mentionQuery: "",
      mentionHits: [],
      error: undefined,
    });
  },
}));

export const workspaceFilesLoading = (
  state: WorkspaceFilesStore,
  relativeDir = WORKSPACE_ROOT_DIR,
): boolean => {
  const dir = normalizeDir(relativeDir);
  return state.loadingDirs.includes(dir) && !state.dirs[dir];
};
