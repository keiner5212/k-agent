import LiquidGlass from "liquid-glass-react";
import type { CSSProperties, ReactNode } from "react";
import { GLASS_SURFACE } from "@/lib/glass";
import { useSettingsStore } from "@/lib/settings";

type GlassSurfaceProps = {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  cornerRadius?: number;
  elasticity?: number;
};

const SURFACE_HEIGHT = "min(88vh, calc(100vh - var(--space-6)))";

const SOLID_STYLE: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  position: "fixed",
  top: "50%",
  left: "50%",
  transform: "translate(-50%, -50%)",
  width: "min(880px, calc(100vw - var(--space-6)))",
  height: SURFACE_HEIGHT,
  maxHeight: "calc(100vh - var(--space-6))",
  zIndex: 51,
};

const LIQUID_STYLE: CSSProperties = {
  position: "fixed",
  top: "50%",
  left: "50%",
  width: "min(880px, calc(100vw - var(--space-6)))",
  height: SURFACE_HEIGHT,
  maxHeight: "calc(100vh - var(--space-6))",
  zIndex: 51,
};

export const GlassSurface = ({
  children,
  className,
  style,
  cornerRadius = 4,
  elasticity = 0,
}: GlassSurfaceProps): ReactNode => {
  const glass = useSettingsStore((state) => state.translucencyEnabled);
  const theme = useSettingsStore((state) => state.theme);

  if (!glass) {
    return (
      <div className={className} style={{ ...SOLID_STYLE, ...style }}>
        {children}
      </div>
    );
  }

  return (
    <LiquidGlass
      displacementScale={GLASS_SURFACE.displacementScale}
      blurAmount={GLASS_SURFACE.blurAmount}
      saturation={GLASS_SURFACE.saturation}
      aberrationIntensity={GLASS_SURFACE.aberrationIntensity}
      elasticity={elasticity}
      cornerRadius={cornerRadius}
      padding="0px"
      overLight={theme === "light"}
      className={className}
      style={{ ...LIQUID_STYLE, ...style }}
    >
      <div className="glass-surface__inner">{children}</div>
    </LiquidGlass>
  );
};
