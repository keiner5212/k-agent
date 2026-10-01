import type { AppLanguage } from "@/types/settings";
import { wrapSection, type PromptShape } from "@/lib/prompt-shape";

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
