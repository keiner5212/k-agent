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

export type Platform = "macos" | "windows" | "linux" | "other";

export const platform = (): Platform => {
  if (typeof navigator === "undefined") return "other";
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("mac")) return "macos";
  if (ua.includes("win")) return "windows";
  if (ua.includes("linux")) return "linux";
  return "other";
};

export const isMac = (): boolean => platform() === "macos";
export const isWindows = (): boolean => platform() === "windows";
export const isLinux = (): boolean => platform() === "linux";

/** Separator used by PATH on this host. */
export const pathSeparator = (): string => (isWindows() ? ";" : ":");

/** Shell the app runs when the user has not chosen one. */
export const defaultShellProgram = (): string => {
  if (isWindows()) return "powershell.exe";
  if (isMac()) return "/bin/zsh";
  return "/bin/sh";
};

/** Name of the host in the words the app shows. */
export const platformLabel = (): string => {
  switch (platform()) {
    case "macos":
      return "macOS";
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
    default:
      return "this host";
  }
};
