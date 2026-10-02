import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { domToBlob } from "modern-screenshot";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type PaletteId = "app" | "blush" | "mist" | "sand";

type Palette = {
  id: PaletteId;
  node: string;
  text: string;
  line: string;
};

const PALETTE_IDS: readonly PaletteId[] = ["app", "blush", "mist", "sand"];

const PASTELS: Record<Exclude<PaletteId, "app">, Palette> = {
  blush: { id: "blush", node: "#f6d5e4", text: "#6b3a4d", line: "#c989a4" },
  mist: { id: "mist", node: "#d4ebf3", text: "#24515f", line: "#6aa8bc" },
  sand: { id: "sand", node: "#f6e7cf", text: "#6a5230", line: "#c4a36a" },
};

const APP_PALETTE: Record<"light" | "dark", Palette> = {
  light: { id: "app", node: "#ffffff", text: "#0e1116", line: "#2a7a8e" },
  dark: { id: "app", node: "#1c2128", text: "#e6eaf2", line: "#5eb6cc" },
};

const resolvePalette = (id: PaletteId, light: boolean): Palette => {
  if (id === "app") return light ? APP_PALETTE.light : APP_PALETTE.dark;
  return PASTELS[id];
};

type MermaidApi = {
  initialize: (config: Record<string, unknown>) => void;
  render: (id: string, text: string) => Promise<{ svg: string }>;
};

let mermaidApi: Promise<MermaidApi> | null = null;
let renderSerial = 0;

const loadMermaid = (): Promise<MermaidApi> => {
  mermaidApi ??= import("mermaid").then((mod) => {
    const api = mod.default as MermaidApi;
    api.initialize({ startOnLoad: false, securityLevel: "strict", theme: "neutral" });
    return api;
  });
  return mermaidApi;
};

const paint = (svg: string, palette: Palette): string => {
  const id = svg.match(/\bid="(kDiagram\d+)"/)?.[1];
  const scope = id ? `#${id}` : "svg";
  const style = `<style>
    ${scope} { background: transparent !important; }
    ${scope} .actor, ${scope} rect.actor, ${scope} polygon.actor, ${scope} rect.note, ${scope} .note,
    ${scope} .labelBox, ${scope} .activation0, ${scope} .activation1, ${scope} .activation2,
    ${scope} .node rect, ${scope} .node polygon, ${scope} .node circle, ${scope} .node ellipse, ${scope} .node path {
      fill: ${palette.node} !important;
      stroke: ${palette.line} !important;
    }
    ${scope} .actor-line, ${scope} .messageLine0, ${scope} .messageLine1, ${scope} .loopLine,
    ${scope} .edgePath path, ${scope} .flowchart-link {
      stroke: ${palette.line} !important;
    }
    ${scope} .arrowheadPath, ${scope} .marker path, ${scope} polygon.arrowhead,
    ${scope} [id$="-arrowhead"] path, ${scope} [id$="-crosshead"] path, ${scope} [id$="-sequencenumber"] circle {
      fill: ${palette.line} !important;
      stroke: ${palette.line} !important;
    }
    ${scope} .cluster rect { fill: transparent !important; stroke: ${palette.line} !important; }
    ${scope} .edgeLabel, ${scope} .edgeLabel p, ${scope} .edgeLabel div, ${scope} .labelBkg,
    ${scope} .edgeLabel rect, ${scope} .edgeLabel .label rect {
      fill: transparent !important;
      background: transparent !important;
      background-color: ${palette.line} !important;
      stroke: none !important;
      opacity: 1 !important;
    }
    ${scope} text, ${scope} tspan, ${scope} .messageText, ${scope} .noteText, ${scope} .loopText,
    ${scope} .labelText, ${scope} .sectionTitle, ${scope} text.actor > tspan,
    ${scope} .nodeLabel, ${scope} .edgeLabel text, ${scope} .edgeLabel span, ${scope} .edgeLabel p,
    ${scope} .nodeLabel span, ${scope} foreignObject div, ${scope} foreignObject span {
      fill: ${palette.text} !important;
      color: ${palette.text} !important;
      stroke: none !important;
    }
    ${scope} .edgeLabel span, ${scope} .edgeLabel text, ${scope} .edgeLabel tspan, ${scope} .edgeLabel p,
    ${scope} .sequenceNumber { fill: ${palette.node} !important; color: ${palette.node} !important; }
  </style>`;
  const close = svg.lastIndexOf("</style>");
  if (close >= 0)
    return svg.slice(0, close + "</style>".length) + style + svg.slice(close + "</style>".length);
  return svg.replace(/<svg\b[^>]*>/, (open) => `${open}${style}`);
};

const SVG_NS = "http://www.w3.org/2000/svg";
const XLINK_NS = "http://www.w3.org/1999/xlink";
const shotOptions = { scale: 2, backgroundColor: null, font: false } as const;

const readDataUrl = (blob: Blob): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") resolve(reader.result);
      else reject(new Error("read"));
    };
    reader.onerror = () => reject(reader.error ?? new Error("read"));
    reader.readAsDataURL(blob);
  });

const embedLabels = async (live: SVGSVGElement, clone: SVGSVGElement): Promise<void> => {
  const sources = [...live.querySelectorAll("foreignObject")];
  const targets = [...clone.querySelectorAll("foreignObject")];
  await Promise.all(
    sources.map(async (source, index) => {
      const target = targets[index];
      const label = source.firstElementChild;
      if (!target || !(label instanceof HTMLElement)) return;
      const shot = await domToBlob(label, shotOptions);
      const href = await readDataUrl(shot);
      const image = document.createElementNS(SVG_NS, "image");
      image.setAttribute("x", source.getAttribute("x") ?? "0");
      image.setAttribute("y", source.getAttribute("y") ?? "0");
      image.setAttribute("width", source.getAttribute("width") ?? `${label.offsetWidth}`);
      image.setAttribute("height", source.getAttribute("height") ?? `${label.offsetHeight}`);
      image.setAttribute("href", href);
      image.setAttributeNS(XLINK_NS, "href", href);
      target.replaceWith(image);
    }),
  );
};

const capturePng = async (host: HTMLElement): Promise<Blob | null> => {
  const live = host.querySelector("svg");
  if (!live) return null;
  const box = live.getBoundingClientRect();
  if (box.width < 1 || box.height < 1) return null;
  const clone = live.cloneNode(true) as SVGSVGElement;
  await embedLabels(live, clone);
  clone.setAttribute("width", String(Math.ceil(box.width)));
  clone.setAttribute("height", String(Math.ceil(box.height)));
  const holder = document.createElement("div");
  holder.style.cssText = `position:fixed;left:-10000px;top:0;width:${box.width}px;height:${box.height}px`;
  holder.append(clone);
  document.body.append(holder);
  try {
    return await domToBlob(clone, shotOptions);
  } catch {
    return null;
  } finally {
    holder.remove();
  }
};

const downloadBlob = (blob: Blob, name: string): void => {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1500);
};

type MermaidPreviewDialogProps = {
  open: boolean;
  source: string;
  onOpenChange: (open: boolean) => void;
};

export const MermaidPreviewDialog = ({
  open,
  source,
  onOpenChange,
}: MermaidPreviewDialogProps): ReactNode => {
  const { t } = useTranslation();
  const shotRef = useRef<HTMLDivElement>(null);
  const [paletteId, setPaletteId] = useState<PaletteId>("app");
  const [rawSvg, setRawSvg] = useState("");
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [copied, setCopied] = useState(false);
  const [downloaded, setDownloaded] = useState(false);
  const light = document.documentElement.getAttribute("data-theme") === "light";
  const palette = resolvePalette(paletteId, light);
  const job = open && source.length > 0 ? source : "";
  const [jobKey, setJobKey] = useState(job);
  if (job !== jobKey) {
    setJobKey(job);
    setLoading(job.length > 0);
    setFailed(false);
    setRawSvg("");
    setCopied(false);
    setDownloaded(false);
  }
  const svg = useMemo(() => (rawSvg.length > 0 ? paint(rawSvg, palette) : ""), [rawSvg, palette]);

  useEffect(() => {
    if (job.length === 0) return;
    let alive = true;
    const id = `kDiagram${++renderSerial}`;
    void (async () => {
      try {
        const api = await loadMermaid();
        const drawn = await api.render(id, job);
        if (!alive) return;
        setRawSvg(drawn.svg);
      } catch {
        if (alive) setFailed(true);
      } finally {
        if (alive) setLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, [job]);

  const onDownload = (): void => {
    const host = shotRef.current;
    if (!host) return;
    void capturePng(host).then((png) => {
      if (!png) return;
      downloadBlob(png, "diagram.png");
      setDownloaded(true);
      window.setTimeout(() => setDownloaded(false), 1600);
    });
  };

  const onCopy = (): void => {
    const host = shotRef.current;
    if (!host || typeof ClipboardItem === "undefined") return;
    void capturePng(host).then(async (png) => {
      if (!png) return;
      try {
        await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1200);
      } catch {
        setCopied(false);
      }
    });
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="chat.diagram.title"
      size="wide"
      footer={
        <div className="diagram-view__bar">
          <div className="diagram-view__palettes">
            {PALETTE_IDS.map((id) => (
              <GlassButton
                key={id}
                variant={id === paletteId ? "primary" : "secondary"}
                onClick={() => setPaletteId(id)}
              >
                {t(`chat.diagram.palettes.${id}`)}
              </GlassButton>
            ))}
          </div>
          <div className="diagram-view__actions">
            <GlassButton variant="secondary" onClick={onCopy} disabled={svg.length === 0}>
              {copied ? t("chat.diagram.copied") : t("chat.diagram.copy")}
            </GlassButton>
            <GlassButton variant="primary" onClick={onDownload} disabled={svg.length === 0}>
              {downloaded ? t("chat.diagram.downloaded") : t("chat.diagram.download")}
            </GlassButton>
          </div>
        </div>
      }
    >
      <div className="diagram-view__stage">
        {loading ? (
          <p className="diagram-view__status">
            <Loader2 size={14} strokeWidth={1.5} className="spin" />
            <span>{t("chat.diagram.loading")}</span>
          </p>
        ) : null}
        {failed ? <p className="diagram-view__status">{t("chat.diagram.error")}</p> : null}
        {svg.length > 0 ? (
          <div
            className="diagram-view__svg"
            ref={shotRef}
            dangerouslySetInnerHTML={{ __html: svg }}
          />
        ) : null}
      </div>
    </Dialog>
  );
};
