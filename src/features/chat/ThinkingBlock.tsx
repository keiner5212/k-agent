import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { ChatMarkdown } from "./ChatMarkdown";
import { useChatStick } from "./use-chat-stick";

const ThinkingDialogBody = ({ reasoning }: { reasoning: string }): ReactNode => {
  const { scrollRef, contentRef } = useChatStick();
  return (
    <div ref={scrollRef} className="chat-thinking-dialog__body">
      <div ref={contentRef}>
        <ChatMarkdown content={reasoning} />
      </div>
    </div>
  );
};

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
        : live
          ? t("chat.thinking.label")
          : t("chat.thinking.done");
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
        <ThinkingDialogBody reasoning={reasoning} />
      </Dialog>
    </>
  );
};
