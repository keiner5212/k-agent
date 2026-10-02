import { useEffect, useState, type ReactNode } from "react";
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

const resolvePalette = (id: PaletteId, light: boolean): Palette => {
  if (id !== "app") return PASTELS[id];
  if (light) return { id: "app", node: "#ffffff", text: "#0e1116", line: "#2a7a8e" };
  return { id: "app", node: "#1c2128", text: "#e6eaf2", line: "#5eb6cc" };
};

type MermaidApi = {
  initialize: (config: Record<string, unknown>) => void;
  render: (id: string, text: string) => Promise<{ svg: string }>;
};

let mermaidApi: Promise<MermaidApi> | null = null;

const loadMermaid = (): Promise<MermaidApi> => {
  mermaidApi ??= import("mermaid").then((mod) => mod.default as MermaidApi);
  return mermaidApi;
};

const paint = (svg: string, palette: Palette): string => {
  const style = `<style>
    svg { background: transparent !important; }
    rect.actor, polygon.actor, rect.note, .labelBox, rect.activation0, rect.activation1, rect.activation2,
    .node rect, .node polygon, .node circle, .node ellipse, .node path {
      fill: ${palette.node} !important;
      stroke: ${palette.line} !important;
    }
    .actor-line, .messageLine0, .messageLine1, .loopLine, .edgePath path, .flowchart-link, line {
      stroke: ${palette.line} !important;
    }
    .arrowheadPath, .marker path, polygon.arrowhead { fill: ${palette.line} !important; stroke: ${palette.line} !important; }
    .cluster rect { fill: transparent !important; stroke: ${palette.line} !important; }
    text.actor, .messageText, .noteText, .loopText, .nodeLabel, .edgeLabel, .label, .nodeLabel span {
      color: ${palette.text} !important;
      fill: ${palette.text} !important;
    }
  </style>`;
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
  const [svg, setSvg] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const light = document.documentElement.getAttribute("data-theme") === "light";
  const palette = resolvePalette(paletteId, light);
  const request = open && source.length > 0 ? `${paletteId}\0${light ? "l" : "d"}\0${source}` : "";
  const [activeRequest, setActiveRequest] = useState(request);
  if (request !== activeRequest) {
    setActiveRequest(request);
    setLoading(request.length > 0);
    setError(null);
    setSvg("");
    setCopied(false);
  }

  useEffect(() => {
    if (request.length === 0) return;
    let alive = true;
    void (async () => {
      try {
        const api = await loadMermaid();
        api.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: "base",
          themeVariables: {
            background: "transparent",
            primaryColor: palette.node,
            primaryTextColor: palette.text,
            primaryBorderColor: palette.line,
            lineColor: palette.line,
            secondaryColor: "transparent",
            tertiaryColor: "transparent",
            mainBkg: palette.node,
            nodeBorder: palette.line,
            clusterBkg: "transparent",
            clusterBorder: palette.line,
            titleColor: palette.text,
            edgeLabelBackground: "transparent",
            textColor: palette.text,
            noteBkgColor: palette.node,
            noteTextColor: palette.text,
            noteBorderColor: palette.line,
            actorBkg: palette.node,
            actorBorder: palette.line,
            actorTextColor: palette.text,
            signalColor: palette.line,
            signalTextColor: palette.text,
          },
        });
        const drawn = await api.render(`diagram${Date.now()}`, source);
        if (!alive) return;
        setSvg(paint(drawn.svg, palette));
      } catch {
        if (alive) setError(t("chat.diagram.error"));
      } finally {
        if (alive) setLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, [request, source, palette, t]);

  const onDownload = (): void => {
    if (svg.length === 0) return;
    void rasterPng(svg).then((png) => {
      if (png) {
        downloadBlob(png, "diagram.png");
        return;
      }
      downloadBlob(new Blob([svg], { type: "image/svg+xml" }), "diagram.svg");
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
              {t("chat.diagram.download")}
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
        {error ? <p className="diagram-view__status">{error}</p> : null}
        {svg.length > 0 ? (
          <div className="diagram-view__svg" dangerouslySetInnerHTML={{ __html: svg }} />
        ) : null}
      </div>
    </Dialog>
  );
};
