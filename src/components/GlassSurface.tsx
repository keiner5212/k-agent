import LiquidGlass from "liquid-glass-react";
import type { CSSProperties, ReactNode } from "react";
import { useSettingsStore } from "@/lib/settings";

type GlassSurfaceProps = {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  cornerRadius?: number;
};

export const GlassSurface = ({
  children,
  className,
  style,
  cornerRadius = 2,
}: GlassSurfaceProps): ReactNode => {
  const translucencyEnabled = useSettingsStore((state) => state.translucencyEnabled);

  if (!translucencyEnabled) {
    return (
      <div className={className} style={style}>
        {children}
      </div>
    );
  }

  return (
    <LiquidGlass
      displacementScale={64}
      blurAmount={0.1}
      saturation={130}
      aberrationIntensity={2}
      elasticity={0.35}
      cornerRadius={cornerRadius}
      className={className}
      style={style}
    >
      {children}
    </LiquidGlass>
  );
};
