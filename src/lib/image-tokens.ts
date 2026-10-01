import type { ProviderKind } from "@/types/providers";

export type VisionModel = {
  kind: ProviderKind;
  modelId: string;
};

const readU32 = (bytes: Uint8Array, offset: number): number =>
  ((bytes[offset] ?? 0) << 24) |
  ((bytes[offset + 1] ?? 0) << 16) |
  ((bytes[offset + 2] ?? 0) << 8) |
  (bytes[offset + 3] ?? 0);

const decodePrefix = (base64: string, maxBytes: number): Uint8Array | null => {
  const clean = base64.replace(/\s+/g, "");
  const chars = Math.min(clean.length - (clean.length % 4), Math.ceil(maxBytes / 3) * 4);
  if (chars < 4) return null;
  try {
    const binary = atob(clean.slice(0, chars));
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
    return out;
  } catch {
    return null;
  }
};

const pngSize = (bytes: Uint8Array): { width: number; height: number } | null => {
  if (bytes.length < 24) return null;
  if (bytes[0] !== 0x89 || bytes[1] !== 0x50) return null;
  const width = readU32(bytes, 16);
  const height = readU32(bytes, 20);
  if (width < 1 || height < 1) return null;
  return { width, height };
};

const jpegSize = (bytes: Uint8Array): { width: number; height: number } | null => {
  if (bytes.length < 4 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return null;
  let offset = 2;
  while (offset + 8 < bytes.length) {
    if (bytes[offset] !== 0xff) {
      offset += 1;
      continue;
    }
    const marker = bytes[offset + 1] ?? 0;
    if (marker === 0xc0 || marker === 0xc1 || marker === 0xc2) {
      const height = ((bytes[offset + 5] ?? 0) << 8) | (bytes[offset + 6] ?? 0);
      const width = ((bytes[offset + 7] ?? 0) << 8) | (bytes[offset + 8] ?? 0);
      if (width < 1 || height < 1) return null;
      return { width, height };
    }
    const size = ((bytes[offset + 2] ?? 0) << 8) | (bytes[offset + 3] ?? 0);
    if (size < 2) return null;
    offset += 2 + size;
  }
  return null;
};

export const imageSizeFromBase64 = (base64: string): { width: number; height: number } | null => {
  const bytes = decodePrefix(base64, 64 * 1024);
  if (!bytes) return null;
  return pngSize(bytes) ?? jpegSize(bytes);
};

const fitLongEdge = (
  width: number,
  height: number,
  max: number,
): { width: number; height: number } => {
  const long = Math.max(width, height);
  if (long <= max) return { width, height };
  const scale = max / long;
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
};

// Anthropic bills (width * height) / 750 after the long edge is capped at 1568.
const anthropicImageTokens = (width: number, height: number): number => {
  const fitted = fitLongEdge(width, height, 1568);
  return Math.ceil((fitted.width * fitted.height) / 750);
};

// Gemini 2.x: 258 tokens when both sides are <= 384, else 258 per 768px tile.
const geminiImageTokens = (width: number, height: number): number => {
  if (width <= 384 && height <= 384) return 258;
  return 258 * Math.ceil(width / 768) * Math.ceil(height / 768);
};

// OpenAI high detail: fit in 2048, shortest side 768, then 512px tiles. 85 + 170 each.
// detail is omitted on the wire, so auto uses this path above 512px and 85 below.
const openaiTileTokens = (width: number, height: number): number => {
  if (width <= 512 && height <= 512) return 85;
  const boxed = fitLongEdge(width, height, 2048);
  let w = boxed.width;
  let h = boxed.height;
  const short = Math.min(w, h);
  if (short > 768) {
    const scale = 768 / short;
    w = Math.max(1, Math.round(w * scale));
    h = Math.max(1, Math.round(h * scale));
  }
  const tiles = Math.ceil(w / 512) * Math.ceil(h / 512);
  return 85 + 170 * tiles;
};

const PATCH = 32;
const MAX_PATCHES = 1536;

const openaiPatchTokens = (width: number, height: number, multiplier: number): number => {
  let patches = Math.ceil(width / PATCH) * Math.ceil(height / PATCH);
  if (patches > MAX_PATCHES && width > 0 && height > 0) {
    const scale = Math.sqrt((MAX_PATCHES * PATCH * PATCH) / (width * height));
    const w = Math.max(1, Math.floor(width * scale));
    const h = Math.max(1, Math.floor(height * scale));
    patches = Math.max(1, Math.ceil(w / PATCH) * Math.ceil(h / PATCH));
    if (patches > MAX_PATCHES) patches = MAX_PATCHES;
  }
  return Math.ceil(patches * multiplier);
};

// Published OpenAI patch multipliers. Tile math stays for gpt-4o and unknown ids.
const patchMultiplier = (modelId: string): number | null => {
  const id = modelId.toLowerCase();
  if (id.includes("gpt-4.1-nano")) return 2.46;
  if (id.includes("gpt-4.1-mini")) return 1.62;
  if (id.includes("o4-mini")) return 1.72;
  if (id.includes("gpt-4.1") || id.includes("gpt-5") || /(^|[^a-z])o[134]/.test(id)) return 1.62;
  return null;
};

export const imageTokens = (width: number, height: number, model: VisionModel | null): number => {
  if (width < 1 || height < 1) return 0;
  const kind = model?.kind ?? "openai-like";
  if (kind === "anthropic-like") return anthropicImageTokens(width, height);
  if (kind === "gemini-like") return geminiImageTokens(width, height);
  const multiplier = model ? patchMultiplier(model.modelId) : null;
  if (multiplier !== null) return openaiPatchTokens(width, height, multiplier);
  return openaiTileTokens(width, height);
};
