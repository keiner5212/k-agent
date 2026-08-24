import { useId, useMemo, type ReactNode } from "react";
import { useSettingsStore } from "@/lib/settings";

// Displacement + chromatic SVG filter adapted from liquid-glass-react (MIT, Max Rovensky).
// Layout patched: fills the host box. No top/left 50% or translate(-50%).

const MAP_WIDTH = 128;
const MAP_HEIGHT = 64;

const smoothStep = (a: number, b: number, t: number): number => {
  const x = Math.max(0, Math.min(1, (t - a) / (b - a)));
  return x * x * (3 - 2 * x);
};

const roundedRectSdf = (x: number, y: number, width: number, height: number, radius: number): number => {
  const qx = Math.abs(x) - width + radius;
  const qy = Math.abs(y) - height + radius;
  const outside = Math.hypot(Math.max(qx, 0), Math.max(qy, 0));
  return Math.min(Math.max(qx, qy), 0) + outside - radius;
};

const buildDisplacementMap = (): string => {
  const canvas = document.createElement("canvas");
  canvas.width = MAP_WIDTH;
  canvas.height = MAP_HEIGHT;
  const context = canvas.getContext("2d");
  if (!context) return "";

  const image = context.createImageData(MAP_WIDTH, MAP_HEIGHT);
  const data = image.data;
  const raw: number[] = [];
  let maxScale = 0;

  for (let y = 0; y < MAP_HEIGHT; y += 1) {
    for (let x = 0; x < MAP_WIDTH; x += 1) {
      const ix = x / MAP_WIDTH - 0.5;
      const iy = y / MAP_HEIGHT - 0.5;
      const edge = roundedRectSdf(ix, iy, 0.3, 0.2, 0.6);
      const scaled = smoothStep(0, 1, smoothStep(0.8, 0, edge - 0.15));
      const dx = (ix * scaled + 0.5) * MAP_WIDTH - x;
      const dy = (iy * scaled + 0.5) * MAP_HEIGHT - y;
      maxScale = Math.max(maxScale, Math.abs(dx), Math.abs(dy));
      raw.push(dx, dy);
    }
  }

  maxScale = Math.max(maxScale, 1);
  let index = 0;
  for (let y = 0; y < MAP_HEIGHT; y += 1) {
    for (let x = 0; x < MAP_WIDTH; x += 1) {
      const dx = raw[index] ?? 0;
      const dy = raw[index + 1] ?? 0;
      index += 2;
      const edge = Math.min(x, y, MAP_WIDTH - x - 1, MAP_HEIGHT - y - 1);
      const factor = Math.min(1, edge / 2);
      const pixel = (y * MAP_WIDTH + x) * 4;
      data[pixel] = Math.max(0, Math.min(255, ((dx * factor) / maxScale + 0.5) * 255));
      data[pixel + 1] = Math.max(0, Math.min(255, ((dy * factor) / maxScale + 0.5) * 255));
      data[pixel + 2] = data[pixel + 1] ?? 0;
      data[pixel + 3] = 255;
    }
  }

  context.putImageData(image, 0, 0);
  return canvas.toDataURL();
};

const DISPLACEMENT_MAP = typeof document === "undefined" ? "" : buildDisplacementMap();

type GlassWarpProps = {
  displacementScale?: number;
  aberrationIntensity?: number;
};

export const GlassWarp = ({
  displacementScale = 20,
  aberrationIntensity = 0.35,
}: GlassWarpProps): ReactNode => {
  const rawId = useId().replace(/:/g, "");
  const filterId = `glass-warp-${rawId}`;
  const mapHref = useMemo(() => DISPLACEMENT_MAP, []);

  return (
    <span className="glass-warp">
      <svg className="glass-warp__svg" aria-hidden="true">
        <defs>
          <filter
            id={filterId}
            x="-20%"
            y="-20%"
            width="140%"
            height="140%"
            colorInterpolationFilters="sRGB"
          >
            <feImage
              href={mapHref}
              x="0"
              y="0"
              width="100%"
              height="100%"
              preserveAspectRatio="xMidYMid slice"
              result="map"
            />
            <feDisplacementMap
              in="SourceGraphic"
              in2="map"
              scale={displacementScale}
              xChannelSelector="R"
              yChannelSelector="G"
              result="red"
            />
            <feColorMatrix
              in="red"
              type="matrix"
              values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0"
              result="redChannel"
            />
            <feDisplacementMap
              in="SourceGraphic"
              in2="map"
              scale={displacementScale - aberrationIntensity * 1.2}
              xChannelSelector="R"
              yChannelSelector="G"
              result="green"
            />
            <feColorMatrix
              in="green"
              type="matrix"
              values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0"
              result="greenChannel"
            />
            <feDisplacementMap
              in="SourceGraphic"
              in2="map"
              scale={displacementScale - aberrationIntensity * 2.4}
              xChannelSelector="R"
              yChannelSelector="G"
              result="blue"
            />
            <feColorMatrix
              in="blue"
              type="matrix"
              values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0"
              result="blueChannel"
            />
            <feBlend in="greenChannel" in2="blueChannel" mode="screen" result="gb" />
            <feBlend in="redChannel" in2="gb" mode="screen" />
          </filter>
        </defs>
      </svg>
      <span className="glass-warp__frost" style={{ filter: mapHref ? `url(#${filterId})` : undefined }} />
      <span className="glass-warp__sheen" />
      <span className="glass-warp__rim" />
    </span>
  );
};

export const GlassFill = ({
  displacementScale,
  aberrationIntensity,
}: GlassWarpProps): ReactNode => {
  const glass = useSettingsStore((state) => state.translucencyEnabled);
  if (!glass) return null;
  return (
    <GlassWarp displacementScale={displacementScale} aberrationIntensity={aberrationIntensity} />
  );
};
