import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@/lib/platform";

export type RepoInfo = {
  path: string;
  isRepo: boolean;
  branch: string | null;
};

const noopInfo: RepoInfo = { path: "", isRepo: false, branch: null };

const sameInfo = (left: RepoInfo, right: RepoInfo): boolean =>
  left.path === right.path && left.isRepo === right.isRepo && left.branch === right.branch;

export const useRepoInfo = (workspacePath: string | null): RepoInfo => {
  const [info, setInfo] = useState<RepoInfo>(noopInfo);

  useEffect(() => {
    if (!isTauri() || !workspacePath) return;
    let cancelled = false;
    const apply = (next: RepoInfo): void => {
      if (cancelled) return;
      setInfo((current) => (sameInfo(current, next) ? current : next));
    };
    const load = (): void => {
      invoke<RepoInfo>("get_repo_info", { path: workspacePath })
        .then(apply)
        .catch((error: unknown) => {
          console.warn("get_repo_info failed", error);
          apply({ path: workspacePath, isRepo: false, branch: null });
        });
    };
    load();
    const unlisten = listen("tauri://focus", () => {
      load();
    });
    return () => {
      cancelled = true;
      void unlisten.then((stop) => stop());
    };
  }, [workspacePath]);

  return info;
};
