import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { runRenderMarkdownJob } from "@/lib/jobs";
import { finishMarkdown, withCodeCopy } from "@/lib/markdown";

const COPY_FEEDBACK_MS = 1200;

const copyText = async (text: string): Promise<boolean> => {
  if (typeof navigator === "undefined" || !navigator.clipboard) return false;
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

type ChatMarkdownProps = {
  content: string;
};

export const ChatMarkdown = ({ content }: ChatMarkdownProps): ReactNode => {
  const { t } = useTranslation();
  const linkHint = t("links.openInBrowserHint");
  const copyLabel = t("chat.message.copyCode");
  const copiedLabel = t("chat.message.copied");
  const [html, setHtml] = useState("");
  const timers = useRef<number[]>([]);

  useEffect(() => {
    if (content.length === 0) return;
    let alive = true;
    const hint = linkHint;
    void runRenderMarkdownJob(content, hint).then((next) => {
      if (!alive) return;
      setHtml(finishMarkdown(next.value, hint));
    });
    return () => {
      alive = false;
    };
  }, [content, linkHint]);

  useEffect(() => {
    const pending = timers.current;
    return () => {
      for (const id of pending) window.clearTimeout(id);
      pending.length = 0;
    };
  }, []);

  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("[data-copy-code]");
    if (!button) return;
    event.preventDefault();
    const pre = button.parentElement?.querySelector("pre");
    const text = (pre?.textContent ?? "").replace(/\n$/, "");
    if (text.length === 0) return;
    void copyText(text).then((ok) => {
      if (!ok || !button.isConnected) return;
      button.classList.remove("is-copied");
      void button.offsetWidth;
      button.classList.add("is-copied");
      button.setAttribute("aria-label", copiedLabel);
      button.setAttribute("title", copiedLabel);
      const id = window.setTimeout(() => {
        if (!button.isConnected) return;
        button.classList.remove("is-copied");
        button.setAttribute("aria-label", copyLabel);
        button.setAttribute("title", copyLabel);
      }, COPY_FEEDBACK_MS);
      timers.current.push(id);
    });
  };

  if (content.length === 0) return null;
  if (html.length === 0) {
    return <div className="chat-message__content">{content}</div>;
  }
  const shown = withCodeCopy(html, copyLabel);
  return (
    <div
      className="chat-message__content chat-message__markdown"
      onClick={onClick}
      dangerouslySetInnerHTML={{ __html: shown }}
    />
  );
};
