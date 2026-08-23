"use client";

import { useEffect, useRef, useState } from "react";
import { API, type HealthResponse, type WsServerMessage } from "@k-agent/shared";
import { getHealth, openSocket, send } from "../lib/api";

type WsStatus = "disconnected" | "connecting" | "open";

export default function HomePage() {
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

  return (
    <main className="container">
      <h1>k-agent</h1>
      <p className="subtitle">Desktop AI agent orchestrator. Empty template.</p>

      <section>
        <h2>REST</h2>
        <button onClick={() => getHealth().then(setHealth).catch(() => setHealth(null))}>
          GET {API.health}
        </button>
        <pre>{health ? JSON.stringify(health, null, 2) : "click to fetch"}</pre>
      </section>

      <section>
        <h2>WebSocket</h2>
        {wsStatus === "open" || wsStatus === "connecting" ? (
          <button className="secondary" onClick={disconnect}>Disconnect</button>
        ) : (
          <button onClick={connect}>Connect</button>
        )}
        <p>
          <span className={`dot ${wsStatus === "open" ? "open" : wsStatus === "connecting" ? "connecting" : "closed"}`} />
          {wsStatus}
        </p>
        <pre>{lastMessage ? JSON.stringify(lastMessage, null, 2) : "no messages yet"}</pre>
      </section>
    </main>
  );
}