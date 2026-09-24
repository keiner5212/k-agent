import type MermaidModule from "mermaid";
import type { MermaidConfig } from "mermaid";

type MermaidRuntime = typeof MermaidModule;

let initialized = false;
let initTheme: "dark" | "default" | null = null;
let idCounter = 0;
let mermaidRuntime: MermaidRuntime | null = null;

const decodeEntities = (input: string): string =>
  input
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");

const ensureMermaid = async (): Promise<MermaidRuntime> => {
  if (!mermaidRuntime) {
    const mod = await import("mermaid");
    mermaidRuntime = mod.default;
  }
  return mermaidRuntime;
};

const currentMermaidTheme = (): "dark" | "default" => {
  if (typeof document !== "undefined") {
    const theme = document.documentElement.dataset.theme;
    if (theme === "light") return "default";
  }
  return "dark";
};

let measureContext: CanvasRenderingContext2D | null | undefined;

const fallbackTextBox = (element: SVGGraphicsElement): SVGRect => {
  const fontSize = parseFloat(getComputedStyle(element).fontSize) || 12;
  const visible = (element.textContent ?? "").replace(/\u200b/g, "");
  let width = fontSize * 0.35;
  if (visible.trim().length > 0) {
    if (measureContext === undefined) {
      measureContext = document.createElement("canvas").getContext("2d");
    }
    if (measureContext) {
      const style = getComputedStyle(element);
      measureContext.font = `${style.fontStyle} ${style.fontWeight} ${fontSize}px ${style.fontFamily}`;
      width = Math.max(measureContext.measureText(visible).width, 1);
    } else {
      width = Math.max(visible.length * fontSize * 0.5, 1);
    }
  }
  return { x: 0, y: -fontSize, width, height: fontSize } as SVGRect;
};

// WebKit getBBox is 0x0 for a space and for U+200B, and mermaid throws on that.
type GetBBox = (this: SVGGraphicsElement, options?: SVGBoundingBoxOptions) => SVGRect;

const patchTextMeasure = (): void => {
  const proto = SVGGraphicsElement.prototype as SVGGraphicsElement & { __kAgentBBox?: boolean };
  if (proto.__kAgentBBox) return;
  const native = Object.getOwnPropertyDescriptor(SVGGraphicsElement.prototype, "getBBox")?.value as
    GetBBox | undefined;
  if (!native) return;
  proto.getBBox = function (this: SVGGraphicsElement, options?: SVGBoundingBoxOptions): SVGRect {
    const box = native.call(this, options);
    if (box.width !== 0 || box.height !== 0) return box;
    const name = this.localName;
    if ((name !== "text" && name !== "tspan") || (this.textContent ?? "").length === 0) return box;
    return fallbackTextBox(this);
  };
  proto.__kAgentBBox = true;
};

const buildConfig = (theme: "dark" | "default"): MermaidConfig => {
  const styles =
    typeof document !== "undefined" ? getComputedStyle(document.documentElement) : null;
  const readVar = (name: string, fallback: string): string => {
    const raw = styles?.getPropertyValue(name).trim();
    return raw && raw.length > 0 ? raw : fallback;
  };
  return {
    startOnLoad: false,
    theme,
    securityLevel: "strict",
    suppressErrorRendering: true,
    fontFamily: readVar("--font-sans", "sans-serif"),
    themeVariables: {
      background: readVar("--surface", "#161a20"),
      primaryColor: readVar("--surface-elevated", "#1c2128"),
      primaryTextColor: readVar("--text-primary", "#e6eaf2"),
      primaryBorderColor: readVar("--border-strong", "#3a4250"),
      secondaryColor: readVar("--surface", "#161a20"),
      tertiaryColor: readVar("--background", "#0e1116"),
      lineColor: readVar("--border-strong", "#3a4250"),
      textColor: readVar("--text-primary", "#e6eaf2"),
      fontSize: "14px",
    },
  };
};

const initIfNeeded = async (): Promise<MermaidRuntime> => {
  const theme = currentMermaidTheme();
  if (initialized && initTheme === theme && mermaidRuntime) return mermaidRuntime;
  const mermaid = await ensureMermaid();
  mermaid.initialize(buildConfig(theme));
  initialized = true;
  initTheme = theme;
  return mermaid;
};

const svgCache = new Map<string, string>();
let renderQueue: Promise<void> = Promise.resolve();

export const resetMermaidTheme = (): void => {
  initialized = false;
  initTheme = null;
  svgCache.clear();
};

const escapeAttr = (value: string): string =>
  value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const dropMermaidScratch = (id: string): void => {
  if (typeof document === "undefined") return;
  document.getElementById(`d${id}`)?.remove();
  document.getElementById(`i${id}`)?.remove();
  document.getElementById(id)?.remove();
  document
    .querySelectorAll("body > div[id^='dmermaid-'], body > iframe[id^='imermaid-'], body > svg")
    .forEach((node) => node.remove());
};

const renderOne = async (source: string): Promise<string> => {
  if (typeof SVGGraphicsElement !== "undefined") patchTextMeasure();
  const mermaid = await initIfNeeded();
  const id = `mermaid-${Date.now().toString(36)}-${(idCounter++).toString(36)}`;
  try {
    const { svg } = await mermaid.render(id, source);
    return svg;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return `<pre class="mermaid-error">${escapeAttr(message)}</pre>`;
  } finally {
    dropMermaidScratch(id);
  }
};

export const renderMermaidBlock = (source: string): Promise<string> => {
  const run = renderQueue.then(() => renderOne(source));
  renderQueue = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
};

export const hashMermaidSource = (source: string): string => {
  let h = 5381;
  for (let i = 0; i < source.length; i += 1) {
    h = ((h << 5) + h + source.charCodeAt(i)) | 0;
  }
  return (h >>> 0).toString(36);
};

export const renderMermaidCached = async (source: string): Promise<string> => {
  const hash = hashMermaidSource(source);
  const hit = svgCache.get(hash);
  if (hit) return hit;
  const svg = await renderMermaidBlock(source);
  if (!svg.includes("mermaid-error")) svgCache.set(hash, svg);
  return svg;
};

const FULLSCREEN_ICON =
  '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/></svg>';

export const decodeMermaidSource = (input: string): string => decodeEntities(input);

export const fillMermaidPlaceholders = async (
  html: string,
  fullscreenLabel: string,
): Promise<string> => {
  const matches = [
    ...html.matchAll(
      /<div class="mermaid-placeholder" data-source="([^"]*)" data-hash="([^"]*)"><\/div>/g,
    ),
  ];
  if (matches.length === 0) return html;
  const label = escapeAttr(fullscreenLabel);
  let out = "";
  let cursor = 0;
  for (const match of matches) {
    const start = match.index ?? 0;
    const source = decodeEntities(match[1] ?? "");
    const hash = match[2] ?? "";
    const svg = await renderMermaidCached(source);
    out += html.slice(cursor, start);
    out += `<div class="mermaid-placeholder" data-source="${match[1] ?? ""}" data-hash="${hash}">${svg}<button type="button" class="mermaid-placeholder__fullscreen" aria-label="${label}" title="${label}">${FULLSCREEN_ICON}</button></div>`;
    cursor = start + match[0].length;
  }
  out += html.slice(cursor);
  return out;
};
