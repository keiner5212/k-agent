import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";

const PERF_THRESHOLD_MS = 1;

export const perfLog = (
  scope: string,
  durationMs: number,
  detail?: Record<string, string | number | boolean>,
): void => {
  if (durationMs < PERF_THRESHOLD_MS || !isTauri()) return;
  const suffix = detail
    ? ` ${Object.entries(detail)
        .map(([key, value]) => `${key}=${String(value)}`)
        .join(" ")}`
    : "";
  void invoke("webview_log", {
    level: "debug",
    message: `[perf] ${scope} ${durationMs.toFixed(1)}ms${suffix}`,
  }).catch(() => undefined);
};
