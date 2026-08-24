import { spawn } from "webview";

export interface WebviewOptions {
  url: string;
  title?: string;
  width?: number;
  height?: number;
}

const LINUX_DEPS_HINT =
  "On Linux, the webview binary needs libwebkit2gtk. Install with: " +
  "Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-0  |  Fedora: sudo dnf install webkit2gtk4.1  |  Arch: sudo pacman -S webkit2gtk-4.1";

export async function openWindow(opts: WebviewOptions): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let child: ReturnType<typeof spawn>;
    try {
      child = spawn({
        url: opts.url,
        title: opts.title ?? "k-agent",
        width: opts.width ?? 1280,
        height: opts.height ?? 800,
      });
    } catch (err) {
      reject(err instanceof Error ? : new Error(String(err)));
      return;
    }

    if (!child || typeof child.on !== "function") {
      reject(new Error("webview spawn returned no child process"));
      return;
    }

    let stderrBuf = "";
    child.stderr?.on("data", (chunk: unknown) => {
      stderrBuf += String(chunk);
    });

    let settled = false;
    const fail = (msg: string) => {
      if (settled) return;
      settled = true;
      reject(new Error(msg + (stderrBuf ? `\nstderr: ${stderrBuf.trim()}` : "")));
    };
    const ok = () => {
      if (settled) return;
      settled = true;
      resolve();
    };

    child.on("error", (e: unknown) =>
      fail(`webview failed to start. ${LINUX_DEPS_HINT}\ncause: ${String(e)}`),
    );
    child.on("exit", (code) => {
      if (code === 0 || code === null) ok();
      else fail(`webview exited with code ${code}. ${LINUX_DEPS_HINT}`);
    });
  });
}