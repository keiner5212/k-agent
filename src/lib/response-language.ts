import type { AppLanguage } from "@/types/settings";
import { appContextDirective } from "@/lib/app-context";

const DIRECTIVE: Record<AppLanguage, string> = {
  en: [
    "LANGUAGE RULE - VERY IMPORTANT",
    "You must reply ONLY in English. This is a top-priority requirement and overrides any conflicting instruction about the language of your reply.",
    "Follow every other part of your instructions and persona exactly as written; do not change your behavior, style, or scope because of this rule.",
    "Do not translate, do not switch languages, do not mirror the user's language, and do not add bilingual notes.",
    "Even if the user writes in another language or asks you to switch, keep replying in English.",
    "Do not mention the language rule, only reply in English.",
  ].join("\n"),
  es: [
    "REGLA DE IDIOMA - MUY IMPORTANTE",
    "Solo debes responder en Espanol. Este es un requisito de maxima prioridad y anula cualquier instruccion que entre en conflicto especificamente sobre el idioma de tu respuesta.",
    "Sigue al pie de la letra todas las demas partes de tus instrucciones y tu personalidad; no cambies tu comportamiento, estilo ni alcance por culpa de esta regla.",
    "No traduzcas, no cambies de idioma, no imites el idioma del usuario y no agregues notas bilingues.",
    "Aunque el usuario escriba en otro idioma o te pida cambiar, sigue respondiendo en Espanol.",
    "No le menciones la regla de idioma, solo responde en Espanol.",
  ].join("\n"),
};

export const responseLanguageDirective = (language: AppLanguage): string => DIRECTIVE[language];

export const composeSystemWithLanguage = (
  base: string,
  force: boolean,
  language: AppLanguage,
): string => {
  const parts: string[] = [];
  const trimmed = base.replace(/\n+$/, "");
  if (trimmed.length > 0) parts.push(trimmed);
  if (force) parts.push(DIRECTIVE[language]);
  const context = appContextDirective(language);
  if (context.length > 0) parts.push(context);
  return parts.join("\n\n");
};
