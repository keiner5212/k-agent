import { DESKTOP_REQUIRED, isTauri } from "@/lib/platform";
import { runLspRequestJob, runResolveLanguageServerJob } from "@/lib/jobs";
import { useSettingsStore } from "@/lib/settings";
import type { ResolvedLanguageServer } from "@/types/language-servers";

export const resolveLanguageServer = async (
  path: string,
): Promise<ResolvedLanguageServer | null> => {
  if (!useSettingsStore.getState().lspEnabled) return null;
  if (!isTauri()) return null;
  return runResolveLanguageServerJob(path);
};

export const lspRequest = async (
  path: string,
  method: string,
  params?: unknown,
): Promise<unknown> => {
  if (!useSettingsStore.getState().lspEnabled) {
    throw new Error("language servers disabled");
  }
  if (!isTauri()) {
    throw new Error(DESKTOP_REQUIRED);
  }
  return runLspRequestJob(path, method, params);
};
