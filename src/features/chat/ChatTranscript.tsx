import { memo, useState, type ReactNode } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { ArrowDown, FileText, Film } from "lucide-react";
import { attachmentPreviewUrl, useHydratedAttachment } from "@/lib/attachments";
import type { ChatAttachment, ChatMessage } from "@/types/chat";
import { AttachmentPreviewDialog } from "./AttachmentPreviewDialog";
import { ChatMarkdown } from "./ChatMarkdown";
import { MessageActions } from "./MessageActions";
import { ThinkingBlock, thinkingIsLive } from "./ThinkingBlock";
import { ToolCallsBlock } from "./ToolCallsBlock";
import { useChatStick } from "./use-chat-stick";

const InterruptedFooter = ({ interrupted }: { interrupted: boolean }): ReactNode => {
  const { t } = useTranslation();
  if (!interrupted) return null;
  return <p className="chat-message__interrupted">{t("chat.interruptedByUser")}</p>;
};

const AttachmentThumb = ({
  item,
  sessionId,
  onOpen,
}: {
  item: ChatAttachment;
  sessionId: string | null;
  onOpen: (item: ChatAttachment) => void;
}): ReactNode => {
  const hydrated = useHydratedAttachment(sessionId, item);
  const thumb = attachmentPreviewUrl(hydrated);
  return (
    <button
      type="button"
      className="chat-message__attachment-open"
      title={item.name}
      onClick={() => onOpen(hydrated)}
    >
      {thumb ? (
        <img src={thumb} alt={item.name} className="chat-message__attachment-image" />
      ) : (
        <span className="chat-message__attachment-file">
          {item.kind === "video" ? (
            <Film size={14} strokeWidth={1.5} />
          ) : (
            <FileText size={14} strokeWidth={1.5} />
          )}
          <span>{item.name}</span>
        </span>
      )}
    </button>
  );
};

const MessageAttachments = ({
  items,
  sessionId,
}: {
  items: ChatAttachment[];
  sessionId: string | null;
}): ReactNode => {
  const [preview, setPreview] = useState<ChatAttachment | null>(null);
  if (items.length === 0) return null;
  return (
    <>
      <ul className="chat-message__attachments">
        {items.map((item) => (
          <li key={item.id} className="chat-message__attachment">
            <AttachmentThumb item={item} sessionId={sessionId} onOpen={setPreview} />
          </li>
        ))}
      </ul>
      <AttachmentPreviewDialog
        sessionId={sessionId}
        item={preview}
        onClose={() => setPreview(null)}
      />
    </>
  );
};

const MessageBody = memo(function MessageBody({
  message,
  sessionId,
}: {
  message: ChatMessage;
  sessionId: string | null;
}): ReactNode {
  if (message.kind === "shell") {
    return (
      <>
        <pre className="chat-message__content chat-message__shell">{message.content}</pre>
        <InterruptedFooter interrupted={message.interrupted ?? false} />
      </>
    );
  }
  if (message.role === "assistant") {
    const rounds = message.toolRounds;
    if (rounds && rounds.length > 0) {
      const lastRoundIndex = rounds.length - 1;
      const showTrailingContent =
        !message.streaming && rounds[rounds.length - 1]?.content !== message.content;
      return (
        <>
          {rounds.map((round, index) => {
            const calls = round.calls ?? [];
            const isLastRound = index === lastRoundIndex;
            return (
              <div key={`round-${index}`}>
                <ThinkingBlock
                  reasoning={round.reasoning}
                  live={thinkingIsLive({
                    streaming: message.streaming,
                    isLast: isLastRound,
                    reasoning: round.reasoning,
                    content: round.content,
                    calls: calls.length,
                    thinkingMs: round.thinkingMs,
                  })}
                  thinkingMs={round.thinkingMs}
                />
                <ChatMarkdown content={round.content ?? ""} />
                <ToolCallsBlock
                  sessionId={sessionId}
                  calls={calls}
                  pending={Boolean(message.streaming) && isLastRound}
                />
              </div>
            );
          })}
          {showTrailingContent ? <ChatMarkdown content={message.content} /> : null}
          <InterruptedFooter interrupted={message.interrupted ?? false} />
        </>
      );
    }
    return (
      <>
        <ThinkingBlock
          reasoning={message.reasoning ?? ""}
          live={thinkingIsLive({
            streaming: message.streaming,
            isLast: true,
            reasoning: message.reasoning ?? "",
            content: message.content,
            calls: message.toolCalls?.length ?? 0,
            thinkingMs: message.thinkingMs,
          })}
          thinkingMs={message.thinkingMs}
        />
        <ToolCallsBlock
          sessionId={sessionId}
          calls={message.toolCalls ?? []}
          pending={Boolean(message.streaming)}
        />
        <ChatMarkdown content={message.content} />
        <InterruptedFooter interrupted={message.interrupted ?? false} />
      </>
    );
  }
  return (
    <>
      <MessageAttachments sessionId={sessionId} items={message.attachments ?? []} />
      {message.content ? <p className="chat-message__content">{message.content}</p> : null}
    </>
  );
});

const formatTurnClock = (ms: number, t: TFunction): string => {
  const total = Math.max(0, Math.round(ms / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) {
    return t("chat.turn.hours", {
      hours,
      minutes: String(minutes).padStart(2, "0"),
    });
  }
  if (minutes > 0) {
    return t("chat.turn.minutes", {
      minutes,
      seconds: String(seconds).padStart(2, "0"),
    });
  }
  return t("chat.turn.seconds", { count: total });
};

const formatTurnDuration = (ms: number, t: TFunction): string =>
  t("chat.turn.ended", { time: formatTurnClock(ms, t) });

const messageClass = (message: ChatMessage, fromParent: boolean): string => {
  const shell = message.kind === "shell" ? " chat-message--shell" : "";
  const streaming = message.streaming ? " chat-message--streaming" : "";
  const interrupted = message.interrupted ? " chat-message--interrupted" : "";
  const role = fromParent ? "parent" : message.role;
  return `chat-message chat-message--${role}${shell}${streaming}${interrupted}`;
};

export const ChatTranscript = ({
  messages,
  sessionId,
  actions = true,
  peer = "user",
  children,
}: {
  messages: ChatMessage[];
  sessionId: string | null;
  actions?: boolean;
  peer?: "user" | "parent";
  children?: ReactNode;
}): ReactNode => {
  const { t } = useTranslation();
  const { scrollRef, contentRef, isAtBottom, scrollToBottom } = useChatStick();

  return (
    <>
      <section ref={scrollRef} className="chat-thread chat-thread--active" aria-live="polite">
        <div ref={contentRef} className="chat-thread__messages">
          {messages.map((message) => {
            const fromParent = peer === "parent" && message.role === "user";
            return (
              <article
                key={message.id}
                className={messageClass(message, fromParent)}
                data-role={fromParent ? "parent" : message.role}
              >
                {fromParent ? (
                  <span className="chat-message__kicker">{t("chat.tools.taskParent")}</span>
                ) : null}
                <MessageBody message={message} sessionId={sessionId} />
                {message.role === "assistant" &&
                message.kind !== "shell" &&
                !message.streaming &&
                typeof message.turnMs === "number" ? (
                  <span className="chat-turn-time">{formatTurnDuration(message.turnMs, t)}</span>
                ) : null}
                {actions ? <MessageActions message={message} /> : null}
              </article>
            );
          })}
          {children}
        </div>
      </section>
      {isAtBottom ? null : (
        <button
          type="button"
          className="chat-jump"
          onClick={() => {
            void scrollToBottom({ animation: "instant" });
          }}
        >
          <ArrowDown size={14} strokeWidth={1.75} aria-hidden="true" />
          <span>{t("chat.thread.jumpToBottom")}</span>
        </button>
      )}
    </>
  );
};
