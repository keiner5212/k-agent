export type Locale = "en" | "es";
export const SUPPORTED_LOCALES: readonly Locale[] = ["en", "es"] as const;

export type Messages = {
  app: { subtitle: string };
  sections: { rest: string; ws: string };
  rest: { button: string; placeholder: string };
  ws: {
    connect: string;
    disconnect: string;
    status: string;
    statusOpen: string;
    statusConnecting: string;
    statusDisconnected: string;
    placeholder: string;
  };
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

export const messages: Record<Locale, Messages> = { en, es };