import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles } from "lucide-react";
import { useAskUserStore } from "@/lib/ask-user";
import { INTERRUPT_ARM_MS, selectActiveMessages, useSessionsStore } from "@/lib/sessions";
import type { ChatMessage } from "@/types/chat";
import { ChatTranscript } from "./ChatTranscript";
import { assistantIsAnswering, ChatWaitingLine, shellIsStreaming } from "./ChatWaitingLine";
import { QuestionDialog } from "./QuestionDialog";
import { TodoList } from "./TodoList";

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

const todosDismissedByLaterUser = (messages: ChatMessage[]): boolean => {
  let lastTodo = -1;
  let lastUser = -1;
  for (let index = 0; index < messages.length; index += 1) {
    const message = messages[index];
    if (!message) continue;
    if (message.role === "user") lastUser = index;
    if (message.todos !== undefined) lastTodo = index;
  }
  return lastTodo >= 0 && lastUser > lastTodo;
};

const ThreadTail = ({ messages }: { messages: ChatMessage[] }): ReactNode => {
  const todos = useSessionsStore((state) => {
    const id = state.activeSessionId;
    for (const session of state.sessions) {
      if (session.id === id) return session.todos;
    }
    return undefined;
  });
  const latestTodos = todosDismissedByLaterUser(messages)
    ? undefined
    : (todos ?? [...messages].reverse().find((message) => message.todos !== undefined)?.todos);
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

  return (
    <ChatTranscript messages={messages} sessionId={sessionId}>
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
    </ChatTranscript>
  );
};

export const ChatThread = (): ReactNode => {
  const { t } = useTranslation();
  const messages = useSessionsStore(selectActiveMessages);
  const sending = useSessionsStore((state) => state.sending);
  const sendingSessionId = useSessionsStore((state) => state.sendingSessionId);
  const shellRunning = useSessionsStore((state) => state.shellRunning);
  const shellRunningSessionId = useSessionsStore((state) => state.shellRunningSessionId);
  const activeSessionId = useSessionsStore((state) => state.activeSessionId);
  const error = useSessionsStore((state) => state.error);
  const canRetry = useSessionsStore((state) => state.canRetry);
  const asking = useAskUserStore((state) =>
    Object.values(state.byCallId).some((item) => item.sessionId === activeSessionId),
  );
  const busy =
    (sending && sendingSessionId === activeSessionId) ||
    (shellRunning && shellRunningSessionId === activeSessionId);
  const waiting =
    busy &&
    !error &&
    !asking &&
    !messages.some((message) => assistantIsAnswering(message) || shellIsStreaming(message));

  if (messages.length === 0 && !error) {
    return (
      <div className="chat-thread-host">
        <section className="chat-thread" aria-live="polite">
          <div className="chat-thread__empty">
            <Sparkles size={20} strokeWidth={1.5} />
            <h2 className="chat-thread__empty-title">{t("chat.thread.emptyTitle")}</h2>
            <p className="chat-thread__empty-description">{t("chat.thread.emptyDescription")}</p>
            {waiting ? <ChatWaitingLine /> : null}
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
