import type { AppLanguage } from "@/types/settings";

type AppContextNote = {
  id: string;
  en: string;
  es: string;
};

const NOTES: AppContextNote[] = [];

export const appContextDirective = (language: AppLanguage): string => {
  if (NOTES.length === 0) return "";
  const body = NOTES.map((note) => note[language]).join("\n");
  return `<app-context>\n${body}\n</app-context>`;
};
