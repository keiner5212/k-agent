import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useSessionsStore } from "@/lib/sessions";
import type { ChatMessage, ChatToolCall } from "@/types/chat";

const CHARS_PER_TOKEN = 4;
const WINDOW_MS = 1000;
const MIN_WINDOW_MS = 300;

type Sample = {
  at: number;
  chars: number;
};

const callChars = (call: ChatToolCall): number => {
  if (call.arguments && call.arguments.length > 0) return call.arguments.length;
  return call.argument?.length ?? 0;
};

const outputChars = (message: ChatMessage): number => {
  const rounds = message.toolRounds;
  if (rounds && rounds.length > 0) {
    let chars = 0;
    for (const round of rounds) {
      chars += round.reasoning.length;
      chars += round.content?.length ?? 0;
      for (const call of round.calls) chars += callChars(call);
    }
    return chars;
  }
  return message.content.length + (message.reasoning?.length ?? 0);
};

export const StreamRate = (): ReactNode => {
  const { t } = useTranslation();
  const chars = useSessionsStore((state) => {
    if (
      !state.sending ||
      !state.sendingSessionId ||
      state.activeSessionId !== state.sendingSessionId
    ) {
      return 0;
    }
    const session = state.sessions.find((item) => item.id === state.sendingSessionId);
    const message = session?.messages.find((item) => item.streaming && item.role === "assistant");
    if (!message || message.kind === "shell") return 0;
    return outputChars(message);
  });
  const samplesRef = useRef<Sample[]>([]);
  const [rate, setRate] = useState<number | null>(null);

  useEffect(() => {
    const samples = samplesRef.current;
    if (chars === 0) samples.length = 0;
    else {
      const previous = samples[samples.length - 1];
      if (previous && chars < previous.chars) samples.length = 0;
      const now = performance.now();
      samples.push({ at: now, chars });
      const cutoff = now - WINDOW_MS;
      while (samples.length > 1 && samples[0].at < cutoff) samples.shift();
    }
    const first = samples[0];
    const last = samples[samples.length - 1];
    let next: number | null = null;
    if (first && last) {
      const elapsed = last.at - first.at;
      if (elapsed >= MIN_WINDOW_MS) {
        const tokens = (last.chars - first.chars) / CHARS_PER_TOKEN;
        const value = Math.round((tokens * 1000) / elapsed);
        if (value > 0) next = value;
      }
    }
    const showTimer = window.setTimeout(() => {
      setRate((current) => (current === next ? current : next));
    }, 0);
    const hideTimer = window.setTimeout(() => setRate(null), WINDOW_MS);
    return () => {
      window.clearTimeout(showTimer);
      window.clearTimeout(hideTimer);
    };
  }, [chars]);

  if (rate === null) return null;
  return (
    <span className="stream-rate" title={t("chat.streamRate.title")}>
      {t("chat.streamRate.label", { rate })}
    </span>
  );
};
