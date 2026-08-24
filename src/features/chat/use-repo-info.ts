import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";

export type RepoInfo = {
  path: string;
  isRepo: boolean;
  branch: string | null;
};

const noopInfo: RepoInfo = { path: "", isRepo: false, branch: null };

export const useRepoInfo = (workspacePath: string | null): RepoInfo => {
  const [info, setInfo] = useState<RepoInfo>(noopInfo);

  useEffect(() => {
    if (!isTauri() || !workspacePath) return;
    let cancelled = false;
    invoke<RepoInfo>("get_repo_info", { path: workspacePath })
      .then((result) => {
        if (!cancelled) setInfo(result);
      })
      .catch((error: unknown) => {
        console.warn("get_repo_info failed", error);
        if (!cancelled) setInfo({ path: workspacePath, isRepo: false, branch: null });
      });
    return () => {
      cancelled = true;
    };
  }, [workspacePath]);

  return info;
};