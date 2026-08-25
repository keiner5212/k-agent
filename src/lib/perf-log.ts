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

export const perfMeasure = async <T>(
  scope: string,
  run: () => Promise<T>,
  detail?: Record<string, string | number | boolean>,
): Promise<T> => {
  const start = performance.now();
  try {
    return await run();
  } finally {
    perfLog(scope, performance.now() - start, detail);
  }
};

export const perfMeasureSync = <T>(
  scope: string,
  run: () => T,
  detail?: Record<string, string | number | boolean>,
): T => {
  const start = performance.now();
  try {
    return run();
  } finally {
    perfLog(scope, performance.now() - start, detail);
  }
};
