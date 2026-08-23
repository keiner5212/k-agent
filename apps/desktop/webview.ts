// WebView wrapper for the desktop shell.
//
// The import below is a placeholder. Pick the webview binding that matches
// your platform and Deno version. Verify it exists before running.
//
// Options to consider:
//   - npm:webview (via Deno's npm: specifier)
//   - https://deno.land/x/webview
//   - A JSR package, if one exists for your Deno version
//
// The shape of the Webview constructor and `.run()` method varies between
// bindings. Adjust the call below to match whichever package you install.

// @ts-ignore - placeholder import; replace with the real package URL.
import { Webview } from "https://deno.land/x/webview@0.8.0/mod.ts";

export interface WebviewOptions {
  url: string;
  title?: string;
  width?: number;
  height?: number;
}

export async function openWindow(opts: WebviewOptions): Promise<void> {
  const wv = new Webview({
    title: opts.title ?? "k-agent",
    url: opts.url,
    width: opts.width ?? 1280,
    height: opts.height ?? 800,
    resizable: true,
    debug: false,
  });
  await wv.run();
}