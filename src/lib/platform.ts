export const DESKTOP_REQUIRED = "Desktop shell required";

const FALLBACK_IPC_ERROR = "Unknown error";

const textFrom = (value: unknown): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

export const ipcErrorMessage = (error: unknown, depth = 4): string => {
  if (error instanceof Error) {
    const message = textFrom(error.message);
    if (message) return message;
  }
  const asString = textFrom(error);
  if (asString) return asString;
  if (depth <= 0 || !error || typeof error !== "object") return FALLBACK_IPC_ERROR;
  const record = error as Record<string, unknown>;
  const fromMessage = textFrom(record.message);
  if (fromMessage) return fromMessage;
  for (const value of Object.values(record)) {
    const nested =
      textFrom(value) ??
      (value && typeof value === "object" ? ipcErrorMessage(value, depth - 1) : undefined);
    if (nested && nested !== FALLBACK_IPC_ERROR) return nested;
  }
  return FALLBACK_IPC_ERROR;
};

export const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const platform = (): "macos" | "windows" | "linux" | "other" => {
  if (typeof navigator === "undefined") return "other";
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("mac")) return "macos";
  if (ua.includes("win")) return "windows";
  if (ua.includes("linux")) return "linux";
  return "other";
};

export const isMac = (): boolean => platform() === "macos";
