import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles } from "lucide-react";

const ROTATE_MS = 2800;

const pickPhraseIndex = (count: number, current: number | null): number => {
  if (count <= 1) return 0;
  let next = Math.floor(Math.random() * count);
  if (current !== null && next === current) {
    next = (next + 1 + Math.floor(Math.random() * (count - 1))) % count;
  }
  return next;
};

export const ChatWaitingLine = (): ReactNode => {
  const { t } = useTranslation();
  const phrases = useMemo((): string[] => {
    const raw: unknown = t("chat.waiting.phrases", { returnObjects: true });
    if (!Array.isArray(raw)) return [];
    return raw.filter((item): item is string => typeof item === "string");
  }, [t]);
  const [index, setIndex] = useState(() => pickPhraseIndex(Math.max(phrases.length, 1), null));

  useEffect(() => {
    if (phrases.length <= 1) return;
    const timer = window.setInterval(() => {
      setIndex((current) => pickPhraseIndex(phrases.length, current));
    }, ROTATE_MS);
    return () => window.clearInterval(timer);
  }, [phrases.length]);

  const phrase = phrases[index] ?? phrases[0];
  if (!phrase) return null;

  return (
    <article className="chat-message chat-message--assistant chat-message--pending">
      <span className="chat-waiting__icon" aria-hidden="true">
        <Sparkles size={15} strokeWidth={1.75} />
      </span>
      <p className="chat-message__content chat-waiting__copy">
        <span key={index} className="chat-waiting__phrase">
          {phrase}
        </span>
      </p>
    </article>
  );
};
