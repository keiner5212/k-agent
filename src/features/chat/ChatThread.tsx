import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { ArrowDown, FileText, Film, Sparkles } from "lucide-react";
import { attachmentPreviewUrl, useHydratedAttachment } from "@/lib/attachments";
import { useAskUserStore } from "@/lib/ask-user";
import { INTERRUPT_ARM_MS, selectActiveMessages, useSessionsStore } from "@/lib/sessions";
import type { ChatAttachment, ChatMessage } from "@/types/chat";
import { AttachmentPreviewDialog } from "./AttachmentPreviewDialog";
import { ChatMarkdown } from "./ChatMarkdown";
import { ChatWaitingLine } from "./ChatWaitingLine";
import { MessageActions } from "./MessageActions";
import { QuestionDialog } from "./QuestionDialog";
import { TodoList } from "./TodoList";
import { ThinkingBlock, thinkingIsLive } from "./ThinkingBlock";
import { ToolCallsBlock } from "./ToolCallsBlock";

const clipDetail = (value: string): string => {
  const flat = value.replace(/\s+/g, " ").trim();
  if (flat.length <= 160) return flat;
  return `${flat.slice(0, 157)}...`;
};

const formatChatError = (
  raw: string,
  t: (key: string, options?: Record<string, unknown>) => string,
): string => {
  const text = raw.trim();
  const lower = text.toLowerCase();
  if (lower === "empty model response") return t("chat.error.empty");
  if (lower.startsWith("http error:")) return t("chat.error.network");
  if (lower.startsWith("parse error:")) return t("chat.error.parse");
  if (lower.startsWith("response interrupted:")) return t("chat.error.interrupted");
  const api = text.match(/^api responded with status (\d+):\s*([\s\S]*)$/i);
  if (api) {
    const status = api[1] ?? "";
    const detail = clipDetail(api[2] ?? "");
    if (detail) return t("chat.error.apiDetail", { status, detail });
    return t("chat.error.api", { status });
  }
  return clipDetail(text);
};

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

const ThreadTail = ({ messages }: { messages: ChatMessage[] }): ReactNode => {
  const todos = useSessionsStore((state) => {
    const id = state.activeSessionId;
    for (const session of state.sessions) {
      if (session.id === id) return session.todos;
    }
    return undefined;
  });
  const latestTodos =
    todos ?? [...messages].reverse().find((message) => message.todos !== undefined)?.todos;
  const questionsByCallId = useAskUserStore((state) => state.byCallId);
  const messageIds = new Set(messages.map((message) => message.id));
  const pending = Object.values(questionsByCallId).filter(
    (state) => state.messageId !== null && messageIds.has(state.messageId),
  );
  if ((!latestTodos || latestTodos.length === 0) && pending.length === 0) return null;
  return (
    <div className="chat-thread__tail">
      {latestTodos && latestTodos.length > 0 ? <TodoList todos={latestTodos} /> : null}
      {pending.length > 0 ? (
        <div className="chat-questions">
          {pending.map((state) => (
            <QuestionDialog key={state.callId} state={state} />
          ))}
        </div>
      ) : null}
    </div>
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

const STICK_PX = 64;
const JUMP_PX = 240;

const assistantIsWriting = (message: ChatMessage): boolean => {
  if (!message.streaming || message.role !== "assistant") return false;
  const rounds = message.toolRounds;
  const last = rounds && rounds.length > 0 ? rounds[rounds.length - 1] : undefined;
  if (!last) {
    return message.content.length > 0 || (message.reasoning?.length ?? 0) > 0;
  }
  if ((last.calls?.length ?? 0) > 0) return false;
  return (last.content?.length ?? 0) > 0 || last.reasoning.length > 0;
};

const InterruptHint = (): ReactNode => {
  const { t } = useTranslation();
  const armed = useSessionsStore((state) => state.interruptArmedAt);
  const sending = useSessionsStore((state) => state.sending);
  const shellRunning = useSessionsStore((state) => state.shellRunning);

  useEffect(() => {
    if (armed === null) return;
    const remain = INTERRUPT_ARM_MS - (Date.now() - armed);
    if (remain <= 0) {
      useSessionsStore.getState().disarmInterrupt();
      return;
    }
    const timer = window.setTimeout(() => {
      useSessionsStore.getState().disarmInterrupt();
    }, remain);
    return () => window.clearTimeout(timer);
  }, [armed]);

  if (armed === null || (!sending && !shellRunning)) return null;
  return (
    <div className="chat-interrupt-hint" role="status" aria-live="polite">
      <span className="chat-interrupt-hint__dot" aria-hidden="true" />
      <span>{t("chat.interruptHint")}</span>
    </div>
  );
};

const ActiveThread = ({
  messages,
  waiting,
  error,
  canRetry,
  sessionId,
}: {
  messages: ChatMessage[];
  waiting: boolean;
  error?: string;
  canRetry: boolean;
  sessionId: string | null;
}): ReactNode => {
  const { t } = useTranslation();
  const [showJump, setShowJump] = useState(false);
  const scrollerRef = useRef<HTMLElement>(null);
  const stickRef = useRef(true);
  const pinningRef = useRef(false);
  const lastScrollTopRef = useRef(0);
  const pinBottom = useCallback((): void => {
    const node = scrollerRef.current;
    if (!node || !stickRef.current) return;
    pinningRef.current = true;
    node.scrollTop = node.scrollHeight;
    lastScrollTopRef.current = node.scrollTop;
    window.requestAnimationFrame(() => {
      const next = scrollerRef.current;
      if (next && stickRef.current) {
        next.scrollTop = next.scrollHeight;
        lastScrollTopRef.current = next.scrollTop;
      }
      window.requestAnimationFrame(() => {
        pinningRef.current = false;
      });
    });
  }, []);
  const onThreadScroll = useCallback((): void => {
    if (pinningRef.current) return;
    const node = scrollerRef.current;
    if (!node) return;
    const top = node.scrollTop;
    const distance = node.scrollHeight - node.clientHeight - top;
    if (top < lastScrollTopRef.current - 1) stickRef.current = distance < STICK_PX;
    else if (distance < STICK_PX) stickRef.current = true;
    lastScrollTopRef.current = top;
    const jumped = distance > JUMP_PX;
    setShowJump((current) => (current === jumped ? current : jumped));
  }, []);
  const jumpToBottom = useCallback((): void => {
    stickRef.current = true;
    setShowJump(false);
    pinBottom();
  }, [pinBottom]);
  useLayoutEffect(() => {
    pinBottom();
  }, [messages, waiting, error, pinBottom]);
  useLayoutEffect(() => {
    const node = scrollerRef.current;
    const target = node?.querySelector(".chat-thread__messages");
    if (!target) return;
    const observer = new ResizeObserver(() => {
      pinBottom();
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [pinBottom]);

  return (
    <>
      <section
        ref={scrollerRef}
        className="chat-thread chat-thread--active"
        aria-live="polite"
        onScroll={onThreadScroll}
      >
        <div className="chat-thread__messages">
          {messages.map((message) => (
            <article
              key={message.id}
              className={`chat-message chat-message--${message.role}${message.kind === "shell" ? " chat-message--shell" : ""}${message.streaming ? " chat-message--streaming" : ""}${message.interrupted ? " chat-message--interrupted" : ""}`}
              data-role={message.role}
            >
              <MessageBody message={message} sessionId={sessionId} />
              <MessageActions message={message} />
            </article>
          ))}
          <ThreadTail messages={messages} />
          <div className="chat-thread__reserve">{waiting ? <ChatWaitingLine /> : null}</div>
          <InterruptHint />
          {error ? (
            <div className="chat-thread__error" role="alert">
              <p className="chat-thread__error-text">{formatChatError(error, t)}</p>
              {canRetry ? (
                <button
                  type="button"
                  className="chat-thread__error-retry"
                  onClick={() => {
                    useSessionsStore.getState().retryLast();
                  }}
                >
                  {t("chat.error.retry")}
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
      </section>
      {showJump ? (
        <button type="button" className="chat-jump" onClick={jumpToBottom}>
          <ArrowDown size={14} strokeWidth={1.75} aria-hidden="true" />
          <span>{t("chat.thread.jumpToBottom")}</span>
        </button>
      ) : null}
    </>
  );
};

export const ChatThread = (): ReactNode => {
  const { t } = useTranslation();
  const messages = useSessionsStore(selectActiveMessages);
  const sending = useSessionsStore((state) => state.sending);
  const sendingSessionId = useSessionsStore((state) => state.sendingSessionId);
  const activeSessionId = useSessionsStore((state) => state.activeSessionId);
  const error = useSessionsStore((state) => state.error);
  const canRetry = useSessionsStore((state) => state.canRetry);
  const waiting =
    sending && sendingSessionId === activeSessionId && !error && !messages.some(assistantIsWriting);

  if (messages.length === 0 && !error) {
    return (
      <div className="chat-thread-host">
        <section className="chat-thread" aria-live="polite">
          <div className="chat-thread__empty">
            <Sparkles size={20} strokeWidth={1.5} />
            <h2 className="chat-thread__empty-title">{t("chat.thread.emptyTitle")}</h2>
            <p className="chat-thread__empty-description">{t("chat.thread.emptyDescription")}</p>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="chat-thread-host">
      <ActiveThread
        key={activeSessionId ?? ""}
        messages={messages}
        waiting={waiting}
        error={error}
        canRetry={canRetry}
        sessionId={activeSessionId}
      />
    </div>
  );
};
