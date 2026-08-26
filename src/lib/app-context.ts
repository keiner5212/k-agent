import type { AppLanguage } from "@/types/settings";

type AppContextNote = {
  id: string;
  en: string;
  es: string;
};

const NOTES: AppContextNote[] = [];

const HEADING: Record<AppLanguage, string> = {
  en: "APP CONTEXT",
  es: "CONTEXTO DE LA APP",
};

export const appContextDirective = (language: AppLanguage): string => {
  if (NOTES.length === 0) return "";
  const body = NOTES.map((note) => note[language]).join("\n");
  return `${HEADING[language]}\n${body}`;
};
