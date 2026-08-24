import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en.json";
import es from "./locales/es.json";
import { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES, type AppLanguage } from "@/types/settings";

const isAppLanguage = (value: string): value is AppLanguage =>
  (SUPPORTED_LANGUAGES as readonly string[]).includes(value);

const detectInitialLanguage = (): AppLanguage => {
  if (typeof navigator !== "undefined" && navigator.language) {
    const primary = navigator.language.split("-")[0]?.toLowerCase();
    if (primary && isAppLanguage(primary)) return primary;
  }
  return DEFAULT_LANGUAGE;
};

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    es: { translation: es },
  },
  lng: detectInitialLanguage(),
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: { escapeValue: false },
  returnNull: false,
});

export default i18n;
