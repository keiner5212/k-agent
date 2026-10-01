import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";

/* eslint-disable no-console -- forwards browser console to the Rust process log */

const formatArg = (value: unknown): string => {
  if (typeof value === "string") return value;
  if (value instanceof Error) return value.message;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
};

const forward = (level: string, args: unknown[]): void => {
  const message = args.map(formatArg).join(" ");
  void invoke("webview_log", { level, message }).catch(() => undefined);
};

export const attachWebviewLogging = (): void => {
  if (!isTauri()) return;
  const native = {
    log: console.log.bind(console),
    info: console.info.bind(console),
    warn: console.warn.bind(console),
    error: console.error.bind(console),
    debug: console.debug.bind(console),
  };

  console.log = (...args: unknown[]) => {
    native.log(...args);
    forward("log", args);
  };
  console.info = (...args: unknown[]) => {
    native.info(...args);
    forward("info", args);
  };
  console.warn = (...args: unknown[]) => {
    native.warn(...args);
    forward("warn", args);
  };
  console.error = (...args: unknown[]) => {
    native.error(...args);
    forward("error", args);
  };
  console.debug = (...args: unknown[]) => {
    native.debug(...args);
    forward("debug", args);
  };
};
