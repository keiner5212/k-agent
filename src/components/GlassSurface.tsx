import type { CSSProperties, ReactNode } from "react";
import { useSettingsStore } from "@/lib/settings";

type GlassSurfaceProps = {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
};

const PANEL_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  position: "fixed",
  inset: 0,
  margin: "auto",
  width: "min(880px, calc(100vw - var(--space-6)))",
  height: "min(88vh, calc(100vh - var(--space-6)))",
  overflow: "hidden",
  zIndex: 51,
};

export const GlassSurface = ({ children, className, style }: GlassSurfaceProps): ReactNode => {
  const glass = useSettingsStore((state) => state.translucencyEnabled);

  return (
    <div
      className={className}
      data-glass={glass ? "true" : "false"}
      style={{ ...PANEL_STYLE, ...style }}
    >
      {children}
    </div>
  );
};
