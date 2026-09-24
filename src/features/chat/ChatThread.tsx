import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { ArrowDown, FileText, Film, Sparkles } from "lucide-react";
import { attachmentPreviewUrl, useHydratedAttachment } from "@/lib/attachments";
import { useAskUserStore } from "@/lib/ask-user";
import { runRenderMarkdownJob } from "@/lib/jobs";
import { finishMarkdown } from "@/lib/markdown";
import {
  decodeMermaidSource,
  fillMermaidPlaceholders,
  resetMermaidTheme,
} from "@/lib/mermaid-render";
import { INTERRUPT_ARM_MS, selectActiveMessages, useSessionsStore } from "@/lib/sessions";
import type { ChatAttachment, ChatMessage } from "@/types/chat";
import { AttachmentPreviewDialog } from "./AttachmentPreviewDialog";
import { ChatWaitingLine } from "./ChatWaitingLine";
import { MessageActions } from "./MessageActions";
import { MermaidFullscreenDialog } from "./MermaidFullscreenDialog";
import { QuestionDialog } from "./QuestionDialog";
import { TodoList } from "./TodoList";
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

const AssistantMarkdown = ({
  content,
  streaming,
  themeEpoch,
  onFullscreenMermaid,
}: {
  content: string;
  streaming?: boolean;
  themeEpoch: number;
  onFullscreenMermaid: (source: string) => void;
}): ReactNode => {
  const { t } = useTranslation();
  const linkHint = t("links.openInBrowserHint");
  const fullscreenLabel = t("chat.mermaid.fullscreenLabel");
  const [html, setHtml] = useState("");

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

  const [viewHtml, setViewHtml] = useState("");
  const [readyKey, setReadyKey] = useState("");
  const renderKey = `${themeEpoch}\n${fullscreenLabel}\n${html}`;

  useEffect(() => {
    if (streaming) return;
    let cancelled = false;
    void fillMermaidPlaceholders(html, fullscreenLabel).then((next) => {
      if (cancelled) return;
      setViewHtml(next);
      setReadyKey(renderKey);
    });
    return () => {
      cancelled = true;
    };
  }, [html, streaming, fullscreenLabel, themeEpoch, renderKey]);

  const onClick = (event: MouseEvent<HTMLDivElement>): void => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const button = target.closest(".mermaid-placeholder__fullscreen");
    if (!button) return;
    const encoded = button.closest(".mermaid-placeholder")?.getAttribute("data-source") ?? "";
    onFullscreenMermaid(decodeMermaidSource(encoded));
  };

  if (content.length === 0) return null;
  if (html.length === 0) {
    return <div className="chat-message__content">{content}</div>;
  }
  return (
    <div
      className="chat-message__content chat-message__markdown"
      onClick={onClick}
      dangerouslySetInnerHTML={{ __html: !streaming && readyKey === renderKey ? viewHtml : html }}
    />
  );
};

const ThinkingBlock = ({
  reasoning,
  streaming,
  thinkingMs,
}: {
  reasoning: string;
  streaming?: boolean;
  thinkingMs?: number;
}): ReactNode => {
  const { t } = useTranslation();
  if (reasoning.length === 0) return null;
  const seconds =
    !streaming && thinkingMs !== undefined && thinkingMs >= 1000
      ? Math.max(1, Math.round(thinkingMs / 1000))
      : undefined;
  const ms =
    !streaming && thinkingMs !== undefined && thinkingMs < 1000
      ? Math.max(0, thinkingMs)
      : undefined;
  const label =
    seconds !== undefined
      ? t("chat.thinking.duration", { count: seconds })
      : ms !== undefined
        ? t("chat.thinking.durationMs", { count: ms })
        : t("chat.thinking.label");
  return (
    <details className="chat-thinking" open={Boolean(streaming)}>
      <summary className="chat-thinking__summary">{label}</summary>
      <pre className="chat-thinking__body">{reasoning}</pre>
    </details>
  );
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

const PendingQuestionsBlock = ({ messageId }: { messageId: string }): ReactNode => {
  const questionsByCallId = useAskUserStore((state) => state.byCallId);
  const pending: ReactNode[] = [];
  for (const state of Object.values(questionsByCallId)) {
    if (state.messageId !== messageId) continue;
    pending.push(<QuestionDialog key={state.callId} state={state} />);
  }
  if (pending.length === 0) return null;
  return <div className="chat-questions">{pending}</div>;
};

const MessageBody = memo(function MessageBody({
  message,
  sessionId,
  themeEpoch,
  onFullscreenMermaid,
}: {
  message: ChatMessage;
  sessionId: string | null;
  themeEpoch: number;
  onFullscreenMermaid: (source: string) => void;
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
    const todoList =
      message.todos && message.todos.length > 0 ? <TodoList todos={message.todos} /> : null;
    if (rounds && rounds.length > 0) {
      const lastRoundIndex = rounds.length - 1;
      const showTrailingContent =
        !message.streaming && rounds[rounds.length - 1]?.content !== message.content;
      return (
        <>
          {todoList}
          {rounds.map((round, index) => {
            const calls = round.calls ?? [];
            const isLastRound = index === lastRoundIndex;
            return (
              <div key={`round-${index}`}>
                <ThinkingBlock
                  reasoning={round.reasoning}
                  streaming={Boolean(message.streaming) && isLastRound}
                  thinkingMs={!message.streaming ? round.thinkingMs : undefined}
                />
                <AssistantMarkdown
                  content={round.content ?? ""}
                  streaming={Boolean(message.streaming) && isLastRound}
                  themeEpoch={themeEpoch}
                  onFullscreenMermaid={onFullscreenMermaid}
                />
                <ToolCallsBlock sessionId={sessionId} calls={calls} />
              </div>
            );
          })}
          {showTrailingContent ? (
            <AssistantMarkdown
              content={message.content}
              streaming={Boolean(message.streaming)}
              themeEpoch={themeEpoch}
              onFullscreenMermaid={onFullscreenMermaid}
            />
          ) : null}
          <PendingQuestionsBlock messageId={message.id} />
          <InterruptedFooter interrupted={message.interrupted ?? false} />
        </>
      );
    }
    return (
      <>
        {todoList}
        <ThinkingBlock
          reasoning={message.reasoning ?? ""}
          streaming={message.streaming}
          thinkingMs={message.thinkingMs}
        />
        <ToolCallsBlock sessionId={sessionId} calls={message.toolCalls ?? []} />
        <PendingQuestionsBlock messageId={message.id} />
        <AssistantMarkdown
          content={message.content}
          themeEpoch={themeEpoch}
          onFullscreenMermaid={onFullscreenMermaid}
        />
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
  themeEpoch,
  onFullscreenMermaid,
}: {
  messages: ChatMessage[];
  waiting: boolean;
  error?: string;
  canRetry: boolean;
  sessionId: string | null;
  themeEpoch: number;
  onFullscreenMermaid: (source: string) => void;
}): ReactNode => {
  const { t } = useTranslation();
  const [showJump, setShowJump] = useState(false);
  const scrollerRef = useRef<HTMLElement>(null);
  const stickRef = useRef(true);
  const pinningRef = useRef(false);
  const pinBottom = useCallback((): void => {
    const node = scrollerRef.current;
    if (!node || !stickRef.current) return;
    pinningRef.current = true;
    node.scrollTop = node.scrollHeight;
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        pinningRef.current = false;
      });
    });
  }, []);
  const onThreadScroll = useCallback((): void => {
    if (pinningRef.current) return;
    const node = scrollerRef.current;
    if (!node) return;
    const distance = node.scrollHeight - node.clientHeight - node.scrollTop;
    stickRef.current = distance < STICK_PX;
    const jumped = distance > JUMP_PX;
    setShowJump((current) => (current === jumped ? current : jumped));
  }, []);
  const jumpToBottom = useCallback((): void => {
    stickRef.current = true;
    setShowJump(false);
    const node = scrollerRef.current;
    if (!node) return;
    pinningRef.current = true;
    node.scrollTop = node.scrollHeight;
    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        pinningRef.current = false;
      });
    });
  }, []);
  useLayoutEffect(() => {
    pinBottom();
  }, [messages, waiting, error, pinBottom]);

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
              <MessageBody
                message={message}
                sessionId={sessionId}
                themeEpoch={themeEpoch}
                onFullscreenMermaid={onFullscreenMermaid}
              />
              <MessageActions message={message} />
            </article>
          ))}
          {waiting ? <ChatWaitingLine /> : null}
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
  const [fullscreenSource, setFullscreenSource] = useState<string | null>(null);
  const [fullscreenOpen, setFullscreenOpen] = useState(false);
  const [mermaidTheme, setMermaidTheme] = useState(0);

  useEffect(() => {
    const node = document.documentElement;
    const observer = new MutationObserver(() => {
      resetMermaidTheme();
      setMermaidTheme((value) => value + 1);
    });
    observer.observe(node, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  }, []);
  const waiting =
    sending && sendingSessionId === activeSessionId && !messages.some(assistantIsWriting);
  const openFullscreenMermaid = useCallback((source: string): void => {
    setFullscreenSource(source);
    setFullscreenOpen(true);
  }, []);

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
        themeEpoch={mermaidTheme}
        onFullscreenMermaid={openFullscreenMermaid}
      />
      <MermaidFullscreenDialog
        source={fullscreenSource}
        open={fullscreenOpen}
        onOpenChange={setFullscreenOpen}
      />
    </div>
  );
};
