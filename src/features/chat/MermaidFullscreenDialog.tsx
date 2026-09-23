import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { renderMermaidCached } from "@/lib/mermaid-render";

type MermaidFullscreenDialogProps = {
  source: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
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

  useEffect(() => {
    if (!open || !source) return;
    const node = svgRef.current;
    if (!node) return;
    let cancelled = false;
    setStatus("rendering");
    setError(null);
    void (async () => {
      try {
        const svg = await renderMermaidCached(source);
        if (cancelled) return;
        node.innerHTML = svg;
        setStatus("idle");
      } catch (err) {
        if (cancelled) return;
        setStatus("error");
        setError(err instanceof Error ? err.message : String(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, source]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="chat.mermaid.fullscreenTitle"
      size="wide"
      placement="center"
      surfaceStyle={{
        width: "min(96vw, calc(100vw - var(--space-6)))",
        maxHeight: "calc(100vh - var(--titlebar-height) - var(--space-6))",
      }}
    >
      {!source ? (
        <p className="mermaid-fullscreen__empty">{t("chat.mermaid.unavailable")}</p>
      ) : (
        <div className="mermaid-fullscreen">
          <div ref={svgRef} className="mermaid-fullscreen__svg" data-status={status} />
          {status === "error" && error ? <pre className="mermaid-error">{error}</pre> : null}
        </div>
      )}
    </Dialog>
  );
};
