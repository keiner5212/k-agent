import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@/lib/platform";

let cached: Promise<string[]> | null = null;

const uniqueSorted = (names: string[]): string[] => {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of names) {
    const name = raw.trim();
    if (name.length === 0 || name.startsWith(".")) continue;
    const key = name.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(name);
  }
  out.sort((a, b) => a.localeCompare(b));
  return out;
};

const queryLocalFonts = async (): Promise<string[]> => {
  const query = (
    globalThis as typeof globalThis & {
      queryLocalFonts?: () => Promise<Array<{ family: string }>>;
    }
  ).queryLocalFonts;
  if (typeof query !== "function") return [];
  try {
    const fonts = await query();
    return uniqueSorted(fonts.map((font) => font.family));
  } catch {
    return [];
  }
};

const load = async (): Promise<string[]> => {
  if (isTauri()) {
    try {
      const names = await invoke<string[]>("list_system_fonts");
      return uniqueSorted(names);
    } catch (error) {
      console.warn("list_system_fonts failed", error);
    }
  }
  return queryLocalFonts();
};

export const listSystemFonts = (): Promise<string[]> => {
  if (!cached) cached = load();
  return cached;
};
