import { spawn } from "webview";

export interface WebviewOptions {
  url: string;
  title?: string;
  width?: number;
  height?: number;
}

export async function openWindow(opts: WebviewOptions): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    try {
      const child = spawn({
        url: opts.url,
        title: opts.title ?? "k-agent",
        width: opts.width ?? 1280,
        height: opts.height ?? 800,
      });
      child.on?.("error", (e: unknown) => reject(e));
      child.on?.("exit", () => resolve());
    } catch (err) {
      reject(err);
    }
  });
}