import { isTauri } from "@/lib/platform";
import { runListSystemFontsJob } from "@/lib/jobs";
import { uniqueSortedNames } from "@/lib/jobs-handlers";

let cached: Promise<string[]> | null = null;

const queryLocalFonts = async (): Promise<string[]> => {
  const query = (
    globalThis as typeof globalThis & {
      queryLocalFonts?: () => Promise<Array<{ family: string }>>;
    }
  ).queryLocalFonts;
  if (typeof query !== "function") return [];
  try {
    const fonts = await query();
    return uniqueSortedNames(fonts.map((font) => font.family));
  } catch {
    return [];
  }
};

const load = async (): Promise<string[]> => {
  if (isTauri()) {
    try {
      return await runListSystemFontsJob();
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
