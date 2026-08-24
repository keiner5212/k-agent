import type { CSSProperties } from "react";

export const GLASS_LAYER_STYLE: CSSProperties = {
  position: "absolute",
  pointerEvents: "none",
};

export const GLASS_BUTTON = {
  displacementScale: 48,
  blurAmount: 0.08,
  saturation: 140,
  aberrationIntensity: 1.2,
  cornerRadius: 8,
  padding: "6px 14px",
} as const;

export const GLASS_SURFACE = {
  displacementScale: 36,
  blurAmount: 0.07,
  saturation: 140,
  aberrationIntensity: 1.4,
} as const;
