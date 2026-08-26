import { openUrl } from "@tauri-apps/plugin-opener";
import { isTauri } from "@/lib/platform";

const EXTERNAL_PROTOCOLS = new Set(["http:", "https:", "mailto:"]);

export const isExternalHref = (href: string): boolean => {
  if (href.length === 0) return false;
  if (href.startsWith("#") || href.startsWith("/") || href.startsWith("?")) return false;
  try {
    const url = new URL(href);
    return EXTERNAL_PROTOCOLS.has(url.protocol);
  } catch {
    return false;
  }
};

export const openExternalUrl = async (href: string): Promise<void> => {
  if (!isTauri()) {
    window.open(href, "_blank", "noopener,noreferrer");
    return;
  }
  await openUrl(href, "browser");
};

const findAnchor = (target: EventTarget | null): HTMLAnchorElement | null => {
  if (!(target instanceof Node)) return null;
  let node: Node | null = target;
  while (node !== null) {
    if (node instanceof HTMLAnchorElement && node.hasAttribute("href")) {
      return node;
    }
    node = node.parentNode;
  }
  return null;
};

export const installExternalLinkInterceptor = (): void => {
  const handler = (event: MouseEvent): void => {
    if (event.defaultPrevented) return;
    if (event.button !== 0) return;
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const anchor = findAnchor(event.target);
    if (!anchor) return;
    const href = anchor.getAttribute("href");
    if (!href || !isExternalHref(href)) return;
    event.preventDefault();
    void openExternalUrl(href).catch((error: unknown) => {
      console.warn("openExternalUrl failed", error);
    });
  };
  document.addEventListener("click", handler);
};
