import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
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
  if (close >= 0) return svg.slice(0, close + "</style>".length) + style + svg.slice(close + "</style>".length);
  return svg.replace(/<svg\b[^>]*>/, (open) => `${open}${style}`);
};

const rasterPng = async (svg: string): Promise<Blob | null> => {
  const blob = new Blob([svg], { type: "image/svg+xml;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  try {
    const img = new Image();
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject(new Error("image"));
      img.src = url;
    });
    const width = img.naturalWidth || 800;
    const height = img.naturalHeight || 600;
    const canvas = document.createElement("canvas");
    canvas.width = width * 2;
    canvas.height = height * 2;
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;
    ctx.scale(2, 2);
    ctx.drawImage(img, 0, 0, width, height);
    return await new Promise((resolve) => canvas.toBlob((next) => resolve(next), "image/png"));
  } catch {
    return null;
  } finally {
    URL.revokeObjectURL(url);
  }
};

const downloadBlob = (blob: Blob, name: string): void => {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
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
    if (svg.length === 0) return;
    void rasterPng(svg).then((png) => {
      if (!png) return;
      downloadBlob(png, "diagram.png");
      setDownloaded(true);
      window.setTimeout(() => setDownloaded(false), 1600);
    });
  };

  const onCopy = (): void => {
    if (svg.length === 0 || typeof ClipboardItem === "undefined") return;
    void rasterPng(svg).then(async (png) => {
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
          <div className="diagram-view__svg" dangerouslySetInnerHTML={{ __html: svg }} />
        ) : null}
      </div>
    </Dialog>
  );
};
