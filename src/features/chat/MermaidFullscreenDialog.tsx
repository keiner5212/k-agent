import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { renderMermaidCached } from "@/lib/mermaid-render";

type MermaidFullscreenDialogProps = {
  source: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

type DiagramFrame = {
  width: number;
  height: number;
  slotWidth: number;
};

const naturalSize = (svg: SVGSVGElement): { width: number; height: number } | null => {
  const box = svg.viewBox.baseVal;
  if (box.width > 0 && box.height > 0) return { width: box.width, height: box.height };
  const bounds = svg.getBBox();
  if (bounds.width > 0 && bounds.height > 0) return { width: bounds.width, height: bounds.height };
  return null;
};

const fitDiagram = (host: HTMLElement, svg: SVGSVGElement): DiagramFrame | null => {
  const natural = naturalSize(svg);
  if (!natural) return null;
  const body = host.closest(".dialog-body");
  const root = host.closest(".dialog-root");
  const header = body?.parentElement?.querySelector(".dialog-header");
  const bodyStyle = body instanceof HTMLElement ? getComputedStyle(body) : null;
  const padX = bodyStyle
    ? Number.parseFloat(bodyStyle.paddingLeft) + Number.parseFloat(bodyStyle.paddingRight)
    : 0;
  const padY = bodyStyle
    ? Number.parseFloat(bodyStyle.paddingTop) + Number.parseFloat(bodyStyle.paddingBottom)
    : 0;
  const headerH = header instanceof HTMLElement ? header.getBoundingClientRect().height : 0;
  const rootBox = root instanceof HTMLElement ? root.getBoundingClientRect() : null;
  const gutter = 8;
  const maxW = Math.max(120, (rootBox?.width ?? window.innerWidth) - gutter - padX);
  const maxH = Math.max(80, (rootBox?.height ?? window.innerHeight) - headerH - padY - gutter);
  const scale = Math.min(1, maxW / natural.width, maxH / natural.height);
  const width = Math.max(1, Math.round(natural.width * scale));
  const height = Math.max(1, Math.round(natural.height * scale));
  return { width, height, slotWidth: width + padX + 2 };
};

const stretchSvg = (svg: SVGSVGElement): void => {
  svg.style.maxWidth = "none";
  svg.style.width = "100%";
  svg.style.height = "100%";
};

export const MermaidFullscreenDialog = ({
  source,
  open,
  onOpenChange,
}: MermaidFullscreenDialogProps): ReactNode => {
  const { t } = useTranslation();
  const svgRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<"idle" | "rendering" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const [frame, setFrame] = useState<DiagramFrame | null>(null);

  useEffect(() => {
    if (!open || !source) return;
    const node = svgRef.current;
    if (!node) return;
    let cancelled = false;
    setStatus("rendering");
    setError(null);
    setFrame(null);
    const apply = (): void => {
      const svgEl = node.querySelector("svg");
      if (!(svgEl instanceof SVGSVGElement)) return;
      stretchSvg(svgEl);
      const next = fitDiagram(node, svgEl);
      if (next) setFrame(next);
    };
    void (async () => {
      try {
        const svg = await renderMermaidCached(source);
        if (cancelled) return;
        node.innerHTML = svg;
        apply();
        setStatus("idle");
      } catch (err) {
        if (cancelled) return;
        setStatus("error");
        setError(err instanceof Error ? err.message : String(err));
      }
    })();
    const onResize = (): void => apply();
    window.addEventListener("resize", onResize);
    return () => {
      cancelled = true;
      window.removeEventListener("resize", onResize);
    };
  }, [open, source]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="chat.mermaid.fullscreenTitle"
      placement="center"
      surfaceStyle={{
        width: frame ? `${frame.slotWidth}px` : "max-content",
        maxWidth: "calc(100vw - var(--space-6))",
        height: "auto",
        maxHeight: "calc(100vh - var(--titlebar-height) - var(--space-6))",
      }}
    >
      {!source ? (
        <p className="mermaid-fullscreen__empty">{t("chat.mermaid.unavailable")}</p>
      ) : (
        <div className="mermaid-fullscreen">
          <div
            ref={svgRef}
            className="mermaid-fullscreen__svg"
            data-status={status}
            style={frame ? { width: frame.width, height: frame.height } : undefined}
          />
          {status === "error" && error ? <pre className="mermaid-error">{error}</pre> : null}
        </div>
      )}
    </Dialog>
  );
};
