import { invoke } from "@tauri-apps/api/core";
import type { AppLanguage } from "@/types/settings";
import { isTauri } from "@/lib/platform";
import { wrapSection, type PromptShape } from "@/lib/prompt-shape";

export const loadHostContext = async (): Promise<string> => {
  if (!isTauri()) return "";
  try {
    return await invoke<string>("host_context");
  } catch (error) {
    console.warn("host_context failed", error);
    return "";
  }
};

export const hostContextSection = (body: string, shape: PromptShape): string =>
  wrapSection("environment", body, shape);

type AppContextNote = {
  id: string;
  en: string;
  es: string;
};

const NOTES: AppContextNote[] = [];

export const appContextDirective = (language: AppLanguage, shape: PromptShape = "xml"): string => {
  if (NOTES.length === 0) return "";
  const body = NOTES.map((note) => note[language]).join("\n");
  return wrapSection("app-context", body, shape);
};
