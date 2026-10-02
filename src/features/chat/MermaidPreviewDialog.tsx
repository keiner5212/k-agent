import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";

type PaletteId = "ink" | "tide" | "dusk" | "paper";

type Palette = {
  id: PaletteId;
  bg: string;
  node: string;
  text: string;
  line: string;
  cluster: string;
};

const PALETTES: readonly Palette[] = [
  {
    id: "ink",
    bg: "#0c0a09",
    node: "#1c1917",
    text: "#faf7f2",
    line: "#e7b089",
    cluster: "#292524",
  },
  {
    id: "tide",
    bg: "#042f2e",
    node: "#115e59",
    text: "#f0fdfa",
    line: "#5eead4",
    cluster: "#134e4a",
  },
  {
    id: "dusk",
    bg: "#1e1b4b",
    node: "#312e81",
    text: "#eef2ff",
    line: "#c4b5fd",
    cluster: "#3730a3",
  },
  {
    id: "paper",
    bg: "#faf7f2",
    node: "#fffdf8",
    text: "#1c1917",
    line: "#0f766e",
    cluster: "#f3ece3",
  },
];

type MermaidApi = {
  initialize: (config: Record<string, unknown>) => void;
  render: (id: string, text: string) => Promise<{ svg: string }>;
};

let mermaidApi: Promise<MermaidApi> | null = null;

const loadMermaid = (): Promise<MermaidApi> => {
  mermaidApi ??= import("mermaid").then((mod) => mod.default as MermaidApi);
  return mermaidApi;
};

const paletteById = (id: PaletteId): Palette =>
  PALETTES.find((item) => item.id === id) ?? PALETTES[0];

const paint = (svg: string, palette: Palette): string => {
  const style = `<style>
    svg { background: ${palette.bg}; }
    .node rect, .node polygon, .node circle, .node ellipse, .node path { fill: ${palette.node} !important; stroke: ${palette.line} !important; }
    .edgePath path, .flowchart-link { stroke: ${palette.line} !important; }
    .arrowheadPath, .marker path { fill: ${palette.line} !important; stroke: ${palette.line} !important; }
    .cluster rect { fill: ${palette.cluster} !important; stroke: ${palette.line} !important; }
    .nodeLabel, .edgeLabel, .label, .nodeLabel span { color: ${palette.text} !important; fill: ${palette.text} !important; }
  </style>`;
  return svg.replace(/<svg\b[^>]*>/, (open) => `${open}${style}`);
};

const rasterPng = async (svg: string, bg: string): Promise<Blob | null> => {
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
    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, width, height);
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
  const [paletteId, setPaletteId] = useState<PaletteId>("ink");
  const [svg, setSvg] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const palette = paletteById(paletteId);
  const request = open && source.length > 0 ? `${paletteId}\0${source}` : "";
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
            background: palette.bg,
            primaryColor: palette.node,
            primaryTextColor: palette.text,
            primaryBorderColor: palette.line,
            lineColor: palette.line,
            secondaryColor: palette.cluster,
            tertiaryColor: palette.bg,
            mainBkg: palette.node,
            nodeBorder: palette.line,
            clusterBkg: palette.cluster,
            clusterBorder: palette.line,
            titleColor: palette.text,
            edgeLabelBackground: palette.bg,
            textColor: palette.text,
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
    void rasterPng(svg, palette.bg).then((png) => {
      if (png) {
        downloadBlob(png, "diagram.png");
        return;
      }
      downloadBlob(new Blob([svg], { type: "image/svg+xml" }), "diagram.svg");
    });
  };

  const onCopy = (): void => {
    if (svg.length === 0 || typeof ClipboardItem === "undefined") return;
    void rasterPng(svg, palette.bg).then(async (png) => {
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
            {PALETTES.map((item) => (
              <GlassButton
                key={item.id}
                variant={item.id === paletteId ? "primary" : "secondary"}
                onClick={() => setPaletteId(item.id)}
              >
                {t(`chat.diagram.palettes.${item.id}`)}
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
      <div className="diagram-view__stage" style={{ background: palette.bg }}>
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
