"use client";

import { useEffect, useRef, useState } from "react";
import { type HealthResponse, type WsServerMessage } from "@k-agent/shared";
import { getHealth, openSocket, send } from "../lib/api";
import { useTranslation } from "../lib/i18n";
import { LanguageSwitcher } from "./components/LanguageSwitcher";

type WsStatus = "disconnected" | "connecting" | "open";

export default function HomePage() {
  const { t } = useTranslation();
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [wsStatus, setWsStatus] = useState<WsStatus>("disconnected");
  const [lastMessage, setLastMessage] = useState<WsServerMessage | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    getHealth().then(setHealth).catch(() => setHealth(null));
  }, []);

  function connect() {
    if (wsRef.current) return;
    setWsStatus("connecting");
    const ws = openSocket({
      onOpen: () => {
        setWsStatus("open");
        send(ws, { type: "ping" });
      },
      onMessage: (msg) => setLastMessage(msg),
      onClose: () => {
        setWsStatus("disconnected");
        wsRef.current = null;
      },
      onError: () => setWsStatus("disconnected"),
    });
    wsRef.current = ws;
  }

  function disconnect() {
    wsRef.current?.close();
    wsRef.current = null;
    setWsStatus("disconnected");
  }

  const statusLabel =
    wsStatus === "open"
      ? t.ws.statusOpen
      : wsStatus === "connecting"
        ? t.ws.statusConnecting
        : t.ws.statusDisconnected;

  return (
    <main className="container">
      <header className="page-header">
        <div>
          <h1>k-agent</h1>
          <p className="subtitle">{t.app.subtitle}</p>
        </div>
        <LanguageSwitcher />
      </header>

      <section>
        <h2>{t.sections.rest}</h2>
        <button onClick={() => getHealth().then(setHealth).catch(() => setHealth(null))}>
          {t.rest.button}
        </button>
        <pre>{health ? JSON.stringify(health, null, 2) : t.rest.placeholder}</pre>
      </section>

      <section>
        <h2>{t.sections.ws}</h2>
        {wsStatus === "open" || wsStatus === "connecting" ? (
          <button className="secondary" onClick={disconnect}>{t.ws.disconnect}</button>
        ) : (
          <button onClick={connect}>{t.ws.connect}</button>
        )}
        <p>
          <span className={`dot ${wsStatus === "open" ? "open" : wsStatus === "connecting" ? "connecting" : "closed"}`} />
          {t.ws.status}: {statusLabel}
        </p>
        <pre>{lastMessage ? JSON.stringify(lastMessage, null, 2) : t.ws.placeholder}</pre>
      </section>
    </main>
  );
}