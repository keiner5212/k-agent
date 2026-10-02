import { useLayoutEffect, useRef, useState, type ReactNode, type UIEvent } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { ChatMarkdown } from "./ChatMarkdown";

const STICK_PX = 48;

export const thinkingIsLive = ({
  streaming,
  isLast,
  reasoning,
  content,
  calls,
  thinkingMs,
}: {
  streaming?: boolean;
  isLast: boolean;
  reasoning: string;
  content?: string;
  calls: number;
  thinkingMs?: number;
}): boolean =>
  Boolean(streaming) &&
  isLast &&
  reasoning.length > 0 &&
  thinkingMs === undefined &&
  (content?.length ?? 0) === 0 &&
  calls === 0;

export const ThinkingBlock = ({
  reasoning,
  live = false,
  thinkingMs,
}: {
  reasoning: string;
  live?: boolean;
  thinkingMs?: number;
}): ReactNode => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const bodyRef = useRef<HTMLDivElement>(null);
  const stickRef = useRef(true);
  useLayoutEffect(() => {
    if (!open) {
      stickRef.current = true;
      return;
    }
    const node = bodyRef.current;
    if (!node || !stickRef.current) return;
    node.scrollTop = node.scrollHeight;
  }, [open, reasoning]);
  if (reasoning.length === 0) return null;
  const seconds =
    !live && thinkingMs !== undefined && thinkingMs >= 1000
      ? Math.max(1, Math.round(thinkingMs / 1000))
      : undefined;
  const ms =
    !live && thinkingMs !== undefined && thinkingMs < 1000 ? Math.max(0, thinkingMs) : undefined;
  const label =
    seconds !== undefined
      ? t("chat.thinking.duration", { count: seconds })
      : ms !== undefined
        ? t("chat.thinking.durationMs", { count: ms })
        : t("chat.thinking.label");
  const onBodyScroll = (event: UIEvent<HTMLDivElement>): void => {
    const node = event.currentTarget;
    const distance = node.scrollHeight - node.clientHeight - node.scrollTop;
    stickRef.current = distance < STICK_PX;
  };
  return (
    <>
      <button
        type="button"
        className="chat-thinking"
        aria-busy={live || undefined}
        onClick={() => setOpen(true)}
      >
        {live ? <span className="chat-live-label">{label}</span> : label}
      </button>
      <Dialog
        open={open}
        onOpenChange={setOpen}
        titleKey="chat.thinking.label"
        size="wide"
        placement="center"
      >
        <div ref={bodyRef} className="chat-thinking-dialog__body" onScroll={onBodyScroll}>
          <ChatMarkdown content={reasoning} />
        </div>
      </Dialog>
    </>
  );
};
