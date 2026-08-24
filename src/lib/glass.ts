import type { CSSProperties } from "react";

export const GLASS_LAYER_STYLE: CSSProperties = {
  position: "absolute",
  inset: 0,
  width: "100%",
  height: "100%",
  pointerEvents: "none",
};

export const GLASS_BUTTON = {
  displacementScale: 20,
  blurAmount: 0.1,
  saturation: 120,
  aberrationIntensity: 0.35,
  cornerRadius: 8,
  padding: "0",
} as const;

export const GLASS_SURFACE = {
  displacementScale: 36,
  blurAmount: 0.07,
  saturation: 140,
  aberrationIntensity: 1.4,
} as const;
