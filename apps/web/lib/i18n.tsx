"use client";

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type Locale = "en" | "es";
export const SUPPORTED_LOCALES: readonly Locale[] = ["en", "es"] as const;

const STORAGE_KEY = "k-agent.locale";
const DEFAULT_LOCALE: Locale = "en";

type Messages = {
  app: { subtitle: string };
  sections: { rest: string; ws: string };
  rest: { button: string; placeholder: string };
  ws: { connect: string; disconnect: string; status: string; statusOpen: string; statusConnecting: string; statusDisconnected: string; placeholder: string };
  languages: { en: string; es: string };
};

const en: Messages = {
  app: { subtitle: "Desktop AI agent orchestrator. Empty template." },
  sections: { rest: "REST", ws: "WebSocket" },
  rest: { button: "GET /api/health", placeholder: "click to fetch" },
  ws: {
    connect: "Connect",
    disconnect: "Disconnect",
    status: "status",
    statusOpen: "open",
    statusConnecting: "connecting",
    statusDisconnected: "disconnected",
    placeholder: "no messages yet",
  },
  languages: { en: "English", es: "Spanish" },
};

const es: Messages = {
  app: { subtitle: "Orquestador de agentes IA de escritorio. Plantilla vacia." },
  sections: { rest: "REST", ws: "WebSocket" },
  rest: { button: "GET /api/health", placeholder: "pulsa para pedir" },
  ws: {
    connect: "Conectar",
    disconnect: "Desconectar",
    status: "estado",
    statusOpen: "conectado",
    statusConnecting: "conectando",
    statusDisconnected: "desconectado",
    placeholder: "sin mensajes aun",
  },
  languages: { en: "Ingles", es: "Espanol" },
};

const messages: Record<Locale, Messages> = { en, es };

function isLocale(value: string | null): value is Locale {
  return value === "en" || value === "es";
}

interface I18nContextValue {
  locale: Locale;
  setLocale: (l: Locale) => void;
  t: Messages;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => {
    if (typeof window === "undefined") return DEFAULT_LOCALE;
    const saved = localStorage.getItem(STORAGE_KEY);
    return isLocale(saved) ? saved : DEFAULT_LOCALE;
  });

  const setLocale = useCallback((l: Locale) => {
    setLocaleState(l);
    localStorage.setItem(STORAGE_KEY, l);
  }, []);

  const value = useMemo<I18nContextValue>(
    () => ({ locale, setLocale, t: messages[locale] }),
    [locale, setLocale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useTranslation(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useTranslation must be used within I18nProvider");
  return ctx;
}