import { create } from "zustand";
import { Channel, invoke } from "@tauri-apps/api/core";
import i18n from "@/i18n";
import { buildAgentsMdRules, composeAgentSystem } from "@/lib/agent-system";
import { resolveAgentMeta } from "@/lib/builtin-agents";
import { useAgentsMdStore } from "@/lib/agents-md";
import { useAgentsStore } from "@/lib/agents";
import { useComposerStore, type ComposerMode } from "@/lib/composer";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { resolveOutgoingMentions } from "@/lib/resolve-outgoing-mentions";
import { notifyResponseFinished } from "@/lib/notifications";
import { composeSystemWithLanguage } from "@/lib/response-language";
import { selectRequest, useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
import { useSkillsStore } from "@/lib/skills";
import {
  appendInterruptedFooter,
  buildShellResultContent,
  formatShellMessage,
  runShellCommand,
  summarizeShellResultForAi,
  type ShellChunk,
} from "@/lib/shell";
import {
  buildContextUsage,
  estimateToolDefinitionTokens,
  loadedSkillNamesFromMessages,
  resolveSelectedModel,
} from "@/lib/context-usage";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { useProvidersStore } from "@/lib/providers";
import {
  applySystemReminder,
  messagesBeforeTail,
  sanitizeSessionsSnapshot,
  sessionMessages,
  summaryTranscript,
  toChatTurns,
} from "@/lib/session-turns";
import { getWorkerCoreSnapshot } from "@/lib/worker-cores";
import {
  parseToolChunkText,
  parseToolResultChunk,
  type AskUserAnswerEntry,
  type AskUserQuestion,
  type AskUserQuestionChunk,
  type ChatAttachment,
  type ChatChunk,
  type ChatChunkKind,
  type ChatMessage,
  type SelectedModel,
  type SendChatResult,
  type TodoDiff,
  type TodoItem,
  type ToolRoundTrace,
} from "@/types/chat";
import { useAskUserStore } from "@/lib/ask-user";
import { notifyAskUser } from "@/lib/notifications";
import {
  activateWorkspace,
  adoptUnscopedSessions,
  sessionInWorkspace,
  sortSessions,
  titleFromFirstMessage,
  workspaceKey,
  type SessionRecord,
  type SessionsSnapshot,
} from "@/types/sessions";

export const INTERRUPT_ARM_MS = 2000;
const STREAM_PAINT_MS = 100;

export type QueuedMessage = {
  id: string;
  sessionId: string;
  text: string;
  mode: ComposerMode;
  attachments?: ChatAttachment[];
};

const previewFromMessages = (messages: ChatMessage[]): string => {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const content = messages[i]?.content.trim();
    if (content && content.length > 0) return content;
  }
  return "";
};

const parseTodoChunk = (raw: string): { todos: TodoItem[]; diff: TodoDiff } | null => {
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (!parsed || typeof parsed !== "object") return null;
    const rawTodos = parsed.todos;
    const rawDiff = parsed.diff;
    if (!Array.isArray(rawTodos)) return null;
    const todos: TodoItem[] = [];
    for (const value of rawTodos) {
      if (!value || typeof value !== "object") return null;
      const item = value as Record<string, unknown>;
      const id = typeof item.id === "string" ? item.id.trim() : "";
      const content = typeof item.content === "string" ? item.content.trim() : "";
      const status = item.status;
      const priority = item.priority;
      if (!id || !content) return null;
      if (
        status !== "pending" &&
        status !== "in_progress" &&
        status !== "completed" &&
        status !== "cancelled"
      ) {
        return null;
      }
      if (typeof priority !== "number" || priority < 0 || priority > 10) return null;
      todos.push({ id, content, status, priority });
    }
    const diff: TodoDiff = rawDiff && typeof rawDiff === "object" ? rawDiff : {};
    return { todos, diff };
  } catch (error) {
    console.warn("todowrite chunk parse failed", error);
    return null;
  }
};

const handleAskUserChunk = (
  raw: string,
  messageId: string,
  sessionId: string,
): AskUserQuestionChunk | null => {
  try {
    const parsed = JSON.parse(raw) as Partial<AskUserQuestionChunk>;
    if (!parsed.callId || !Array.isArray(parsed.questions)) return null;
    const questions = parsed.questions.filter(
      (question): question is AskUserQuestion =>
        Boolean(question) &&
        typeof question.id === "string" &&
        typeof question.header === "string" &&
        typeof question.question === "string" &&
        Array.isArray(question.options),
    );
    if (questions.length === 0) return null;
    useAskUserStore.getState().upsert({
      callId: parsed.callId,
      messageId,
      sessionId,
      questions,
      answers: questions.map((question) => ({
        questionId: question.id,
        selected: [],
        freeText: "",
      })),
    });
    const first = questions[0];
    void notifyAskUser(questions.length, first ? first.question : null);
    return {
      callId: parsed.callId,
      questions,
      arguments: typeof parsed.arguments === "string" ? parsed.arguments : undefined,
      thoughtSignature:
        typeof parsed.thoughtSignature === "string" ? parsed.thoughtSignature : undefined,
    };
  } catch (error) {
    console.warn("ask_user chunk parse failed", error);
    return null;
  }
};

const restorePendingAsks = (sessions: SessionRecord[]): void => {
  for (const session of sessions) {
    for (const message of sessionMessages(session)) {
      const pending = message.pendingAsk;
      if (!pending?.callId || pending.questions.length === 0) continue;
      useAskUserStore.getState().upsert({
        callId: pending.callId,
        messageId: message.id,
        sessionId: session.id,
        questions: pending.questions,
        answers: pending.questions.map((question) => ({
          questionId: question.id,
          selected: [],
          freeText: "",
        })),
      });
    }
  }
};

export const answerSummary = (
  questions: AskUserQuestion[],
  answers: AskUserAnswerEntry[],
): string =>
  questions
    .map((question) => {
      const entry = answers.find((item) => item.questionId === question.id);
      if (!entry || entry.skipped) return `${question.header}: (skipped)`;
      const parts = [entry.selected.join(", "), entry.freeText.trim()].filter(
        (part) => part.length > 0,
      );
      return `${question.header}: ${parts.join(" | ") || "(no selection)"}`;
    })
    .join("\n");

const thinkingDurationMs = (
  startedAt: number | undefined,
  endedAt: number | undefined,
): number | undefined => {
  if (startedAt === undefined) return undefined;
  return Math.max(0, (endedAt ?? Date.now()) - startedAt);
};

const resolveUserMessageContent = async (text: string): Promise<string> => {
  if (!text.includes("@") || !isTauri()) return text;
  const workspaceRoot = await invoke<string | null>("get_workspace_path").catch(() => null);
  if (!workspaceRoot) return text;
  return resolveOutgoingMentions(text, workspaceRoot, "plain");
};

const nextId = (): string =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2)}`;

const workspacePathNow = (): string | undefined => {
  const key = workspaceKey(useSkillsStore.getState().workspacePath);
  return key.length > 0 ? key : undefined;
};

const readWorkspacePath = async (): Promise<string | undefined> => {
  if (!isTauri()) return undefined;
  try {
    const path = await invoke<string | null>("get_workspace_path");
    const key = workspaceKey(path);
    return key.length > 0 ? key : undefined;
  } catch {
    return undefined;
  }
};

const emptySession = (workspacePath?: string): SessionRecord => {
  const key = workspaceKey(workspacePath) || workspacePathNow();
  return {
    id: nextId(),
    title: "",
    preview: "",
    updatedAt: Date.now(),
    messages: [],
    ...(key ? { workspacePath: key } : {}),
  };
};

let workspaceEpoch = 0;

const snapshotFromState = (
  sessions: SessionRecord[],
  activeSessionId: string,
): SessionsSnapshot => ({
  activeSessionId,
  sessions,
});

const persistableSnapshot = (snapshot: SessionsSnapshot): SessionsSnapshot => ({
  ...snapshot,
  sessions: snapshot.sessions.map((session) => ({
    ...session,
    messages: sessionMessages(session)
      .filter(
        (message) =>
          !message.streaming || Boolean(message.pendingAsk) || Boolean(message.resumeTools),
      )
      .map((message) => {
        const { toolCalls: _toolCalls, streaming: _streaming, ...rest } = message;
        return message.pendingAsk ? { ...rest, pendingAsk: message.pendingAsk } : rest;
      }),
  })),
});

const restoreTodos = (sessions: SessionRecord[]): void => {
  // Todos are session-scoped. Restore the latest snapshot into each session
  // message so the UI can show the most recent list inline.
  for (const session of sessions) {
    const stored = session.todos;
    if (!stored || stored.length === 0) continue;
    const messages = session.messages;
    let attached = false;
    for (let index = messages.length - 1; index >= 0; index -= 1) {
      const message = messages[index];
      if (!message || message.role !== "assistant") continue;
      if (!message.todos || message.todos.length === 0) {
        message.todos = stored.map((item) => ({ ...item }));
        attached = true;
      }
      break;
    }
    if (!attached && messages.length > 0) {
      const last = messages[messages.length - 1];
      if (last) last.todos = stored.map((item) => ({ ...item }));
    }
  }
};

const HYDRATE_TIMEOUT_MS = 8000;

const invokeWithTimeout = async <T>(command: string, ms: number): Promise<T> => {
  let timer = 0;
  try {
    return await Promise.race([
      invoke<T>(command),
      new Promise<T>((_, reject) => {
        timer = window.setTimeout(() => reject(new Error("sessions hydrate timeout")), ms);
      }),
    ]);
  } finally {
    window.clearTimeout(timer);
  }
};

const persistSnapshot = async (snapshot: SessionsSnapshot): Promise<void> => {
  if (!isTauri()) return;
  try {
    await invoke("save_sessions", { snapshot: persistableSnapshot(snapshot) });
  } catch (error) {
    console.warn("sessions persist failed", error);
  }
};

const resolveTitleModel = (): SelectedModel | null => {
  const { titleGenerationModel } = useSettingsStore.getState();
  if (titleGenerationModel) return titleGenerationModel;
  return useSelectionStore.getState().selection;
};

const resolveSendRequest = () => {
  const request = selectRequest(useSelectionStore.getState());
  return {
    reasoningMode: request.reasoningMode ?? null,
    effort: request.effort ?? null,
    serviceTier: request.serviceTier ?? null,
    temperature: request.temperature ?? null,
    limitProviderDataUse: useSettingsStore.getState().limitProviderDataUse,
  };
};

const compactHistory = async (
  sessionId: string,
  messages: ChatMessage[],
  system: string,
  toolNames: readonly string[],
  selection: SelectedModel,
): Promise<ChatMessage[]> => {
  const { contextSummarizePercent, limitProviderDataUse } = useSettingsStore.getState();
  const { head, tail } = messagesBeforeTail(messages);
  if (head.length === 0 || contextSummarizePercent <= 0) return messages;
  const model = resolveSelectedModel(useProvidersStore.getState().providers, selection);
  const windowTokens = model?.contextWindow;
  if (!windowTokens || windowTokens <= 0) return messages;
  const usage = buildContextUsage({
    windowTokens,
    messages,
    cost: undefined,
    extras: {
      systemPrompt: estimateTokensFromText(system),
      toolDefinitions: estimateToolDefinitionTokens(toolNames),
    },
  });
  if (usage.percent < contextSummarizePercent) return messages;
  const transcript = summaryTranscript(head);
  if (transcript.length === 0) return messages;
  try {
    const result = await invoke<{ summary: string }>("summarize_conversation", {
      input: {
        providerId: selection.providerId,
        modelId: selection.modelId,
        transcript,
        limitProviderDataUse,
      },
    });
    const summary = result.summary.trim();
    if (summary.length === 0) return messages;
    const nextMessages: ChatMessage[] = [
      {
        id: nextId(),
        role: "user",
        content: `<conversation-summary>\n${summary}\n</conversation-summary>`,
      },
      ...tail,
    ];
    const state = useSessionsStore.getState();
    const nextSessions = patchActiveSession(state.sessions, sessionId, (session) => ({
      ...session,
      messages: nextMessages,
    }));
    useSessionsStore.setState({ sessions: nextSessions });
    void persistSnapshot(snapshotFromState(nextSessions, state.activeSessionId ?? sessionId));
    return nextMessages;
  } catch (error) {
    console.warn("context summarize failed", error);
    return messages;
  }
};

const generateSessionTitle = async (firstMessage: string): Promise<string> => {
  const { titleUseFirstMessage } = useSettingsStore.getState();
  if (titleUseFirstMessage) return titleFromFirstMessage(firstMessage);

  const model = resolveTitleModel();
  if (!model || !isTauri()) return titleFromFirstMessage(firstMessage);

  try {
    const result = await invoke<{ title: string }>("generate_session_title", {
      input: {
        providerId: model.providerId,
        modelId: model.modelId,
        message: firstMessage,
        limitProviderDataUse: useSettingsStore.getState().limitProviderDataUse,
      },
    });
    const title = result.title.trim();
    return title.length > 0 ? title : titleFromFirstMessage(firstMessage);
  } catch (error) {
    console.warn("session title generation failed", error);
    return titleFromFirstMessage(firstMessage);
  }
};

const patchActiveSession = (
  sessions: SessionRecord[],
  activeSessionId: string,
  patch: (session: SessionRecord) => SessionRecord,
): SessionRecord[] =>
  sessions.map((session) => (session.id === activeSessionId ? patch(session) : session));

const isBlankSession = (session: SessionRecord): boolean =>
  sessionMessages(session).length === 0 && session.title.trim().length === 0;

const ensureSession = (
  sessions: SessionRecord[],
  activeSessionId: string | null,
): { sessions: SessionRecord[]; activeSessionId: string } => {
  const current = activeSessionId
    ? sessions.find((session) => session.id === activeSessionId)
    : undefined;
  if (current) return { sessions, activeSessionId: current.id };
  const session = emptySession();
  return { sessions: sortSessions([session, ...sessions]), activeSessionId: session.id };
};

export type ConfirmChoice = "deny" | "once" | "session" | "answer";

type ReplayAsk = {
  assistantId: string;
  resumeConfirmed: boolean;
};

const withoutAskFlags = (message: ChatMessage): ChatMessage => {
  const { pendingAsk: _pending, resumeTools: _resume, ...rest } = message;
  return rest;
};

export const confirmChoiceFrom = (
  questions: AskUserQuestion[],
  answers: AskUserAnswerEntry[],
): ConfirmChoice => {
  const confirm = questions.find(
    (question) => question.id === "outside_confirm" || question.id === "http_write_confirm",
  );
  if (!confirm) return "answer";
  const entry = answers.find((item) => item.questionId === confirm.id);
  if (!entry || entry.skipped) return "deny";
  if (entry.selected.includes("Accept for this chat")) return "session";
  if (entry.selected.includes("Accept this time")) return "once";
  return "deny";
};

let sendEpoch = 0;

const continueResumedTools = (sessions: SessionRecord[]): void => {
  if (!isTauri()) return;
  for (const session of sessions) {
    for (const message of sessionMessages(session)) {
      if (!message.resumeTools || message.pendingAsk) continue;
      const calls = (message.toolRounds ?? []).flatMap((round) => round.calls ?? []);
      const missingOutput = calls.some((call) => call.output === undefined);
      const hasReply = message.content.trim().length > 0 && !missingOutput;
      if (hasReply || calls.length === 0) continue;
      const sessionId = session.id;
      const assistantId = message.id;
      void (async () => {
        try {
          await invoke("cancel_running_task", { sessionId });
        } catch (error) {
          console.warn("cancel_running_task failed", error);
        }
        await useSessionsStore.getState().send("", sessionId, undefined, {
          assistantId,
          resumeConfirmed: missingOutput,
        });
      })();
    }
  }
};

type SessionsStore = {
  sessions: SessionRecord[];
  activeSessionId: string | null;
  sendingSessionId: string | null;
  shellRunningSessionId: string | null;
  hydrated: boolean;
  sending: boolean;
  shellRunning: boolean;
  interruptArmedAt: number | null;
  queued: QueuedMessage[];
  error?: string;
  hydrate: () => Promise<void>;
  focusWorkspace: () => Promise<void>;
  create: () => void;
  select: (id: string) => void;
  remove: (id: string) => Promise<void>;
  send: (
    text: string,
    sessionId?: string,
    attachments?: ChatAttachment[],
    replay?: ReplayAsk,
  ) => Promise<boolean>;
  allowOutsideWorkspace: (sessionId: string) => void;
  allowHttpWrite: (sessionId: string) => void;
  clearPendingAsk: (callId: string) => void;
  resumeAsk: (callId: string, text: string, choice: ConfirmChoice) => Promise<boolean>;
  runShell: (text: string, sessionId?: string) => Promise<boolean>;
  enqueue: (text: string, mode: ComposerMode, attachments?: ChatAttachment[]) => void;
  removeQueued: (id: string) => void;
  flushQueued: () => void;
  sendQueuedNow: (id?: string) => Promise<void>;
  interruptActiveTask: () => Promise<boolean>;
  armInterrupt: () => void;
  disarmInterrupt: () => void;
  rewindTo: (messageId: string) => void;
  rewindLastUserMessage: () => void;
  editQueued: (id: string) => void;
};

export const useSessionsStore = create<SessionsStore>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  sendingSessionId: null,
  shellRunningSessionId: null,
  hydrated: false,
  sending: false,
  shellRunning: false,
  interruptArmedAt: null,
  queued: [],

  hydrate: async () => {
    if (get().hydrated) return;
    if (!isTauri()) {
      const session = emptySession();
      set({
        sessions: [session],
        activeSessionId: session.id,
        hydrated: true,
      });
      return;
    }
    const epoch = workspaceEpoch;
    try {
      const snapshot = sanitizeSessionsSnapshot(
        await invokeWithTimeout<SessionsSnapshot>("load_sessions", HYDRATE_TIMEOUT_MS),
      );
      if (get().hydrated) return;
      const path = await readWorkspacePath();
      if (get().hydrated) return;
      const adopted = adoptUnscopedSessions(snapshot.sessions, path);
      const focused = activateWorkspace(adopted.sessions, snapshot.activeSessionId, path, nextId);
      set({
        sessions: focused.sessions,
        activeSessionId: focused.activeSessionId,
        hydrated: true,
      });
      if (adopted.changed || focused.changed) {
        void persistSnapshot(snapshotFromState(focused.sessions, focused.activeSessionId));
      }
      restorePendingAsks(focused.sessions);
      restoreTodos(focused.sessions);
      continueResumedTools(focused.sessions);
      if (epoch !== workspaceEpoch) await get().focusWorkspace();
    } catch (error) {
      console.warn("sessions hydrate failed", error);
      if (get().hydrated) return;
      const session = emptySession();
      set({
        sessions: [session],
        activeSessionId: session.id,
        hydrated: true,
      });
    }
  },

  focusWorkspace: async () => {
    const epoch = (workspaceEpoch += 1);
    if (!get().hydrated) return;
    const path = await readWorkspacePath();
    if (epoch !== workspaceEpoch || !get().hydrated) return;
    const focused = activateWorkspace(get().sessions, get().activeSessionId, path, nextId);
    if (!focused.changed) return;
    set({
      sessions: focused.sessions,
      activeSessionId: focused.activeSessionId,
      error: undefined,
    });
    void persistSnapshot(snapshotFromState(focused.sessions, focused.activeSessionId));
  },

  create: () => {
    const { sessions, activeSessionId } = get();
    const current = activeSessionId
      ? sessions.find((session) => session.id === activeSessionId)
      : undefined;
    if (current && isBlankSession(current) && sessionInWorkspace(current, workspacePathNow())) {
      set({ activeSessionId: current.id, error: undefined });
      return;
    }
    const session = emptySession();
    const next = sortSessions([session, ...sessions]);
    set({ sessions: next, activeSessionId: session.id, error: undefined });
    void persistSnapshot(snapshotFromState(next, session.id));
  },

  select: (id) => {
    if (!get().sessions.some((session) => session.id === id)) return;
    set({ activeSessionId: id, error: undefined });
    void persistSnapshot(snapshotFromState(get().sessions, id));
  },

  remove: async (id) => {
    const { sessions, activeSessionId, sendingSessionId, shellRunningSessionId } = get();
    const nextSessions = sessions.filter((session) => session.id !== id);
    const stopSending = sendingSessionId === id;
    const stopShell = shellRunningSessionId === id;
    if (stopSending || stopShell) {
      try {
        await invoke<boolean>("cancel_running_task", { sessionId: id });
      } catch (error) {
        console.warn("cancel_running_task failed", error);
      }
    }
    const here = nextSessions.filter((session) => sessionInWorkspace(session, workspacePathNow()));
    if (here.length === 0) {
      const session = emptySession();
      const seeded = sortSessions([session, ...nextSessions]);
      set({
        sessions: seeded,
        activeSessionId: session.id,
        error: undefined,
        queued: get().queued.filter((item) => item.sessionId !== id),
        ...(stopSending ? { sending: false, sendingSessionId: null } : {}),
        ...(stopShell ? { shellRunning: false, shellRunningSessionId: null } : {}),
      });
      void persistSnapshot(snapshotFromState(seeded, session.id));
      if (stopSending || stopShell) get().flushQueued();
      return;
    }
    const sorted = sortSessions(nextSessions);
    const sortedHere = sortSessions(here);
    const fallbackId = sortedHere[0]?.id;
    if (!fallbackId) return;
    const nextActiveId =
      activeSessionId !== null &&
      activeSessionId !== id &&
      sortedHere.some((session) => session.id === activeSessionId)
        ? activeSessionId
        : fallbackId;
    set({
      sessions: sorted,
      activeSessionId: nextActiveId,
      error: undefined,
      queued: get().queued.filter((item) => item.sessionId !== id),
      ...(stopSending ? { sending: false, sendingSessionId: null } : {}),
      ...(stopShell ? { shellRunning: false, shellRunningSessionId: null } : {}),
    });
    void persistSnapshot(snapshotFromState(sorted, nextActiveId));
    if (stopSending || stopShell) get().flushQueued();
  },

  allowOutsideWorkspace: (sessionId) => {
    const nextSessions = get().sessions.map((session) =>
      session.id === sessionId ? { ...session, outsideWorkspaceAllowed: true } : session,
    );
    set({ sessions: nextSessions });
    void persistSnapshot(snapshotFromState(nextSessions, get().activeSessionId ?? sessionId));
  },

  allowHttpWrite: (sessionId) => {
    const nextSessions = get().sessions.map((session) =>
      session.id === sessionId ? { ...session, httpWriteAllowed: true } : session,
    );
    set({ sessions: nextSessions });
    void persistSnapshot(snapshotFromState(nextSessions, get().activeSessionId ?? sessionId));
  },

  clearPendingAsk: (callId) => {
    let sessionId: string | null = null;
    const nextSessions = get().sessions.map((session) => ({
      ...session,
      messages: session.messages.map((message) => {
        if (message.pendingAsk?.callId !== callId) return message;
        sessionId = session.id;
        const { pendingAsk: _pending, ...rest } = message;
        return { ...rest, resumeTools: true };
      }),
    }));
    if (!sessionId) return;
    set({ sessions: nextSessions });
    void persistSnapshot(snapshotFromState(nextSessions, get().activeSessionId ?? sessionId));
  },

  resumeAsk: async (callId, text, choice) => {
    let sessionId: string | null = null;
    let assistantId: string | null = null;
    const denyText = callId.startsWith("http_write_confirm::")
      ? "User denied the HTTP write."
      : "User denied access outside the workspace.";
    const nextSessions = get().sessions.map((session) => ({
      ...session,
      messages: session.messages.map((message) => {
        if (message.pendingAsk?.callId !== callId) return message;
        sessionId = session.id;
        assistantId = message.id;
        const toolRounds = (message.toolRounds ?? []).map((round) => ({
          ...round,
          calls: round.calls.map((call) => {
            if (choice === "answer" && call.id === callId) return { ...call, output: text };
            if (choice === "deny" && call.output === undefined) {
              return { ...call, output: denyText };
            }
            return call;
          }),
        }));
        return { ...withoutAskFlags(message), toolRounds, streaming: false };
      }),
    }));
    if (!sessionId || !assistantId) return false;
    set({ sessions: nextSessions });
    void persistSnapshot(snapshotFromState(nextSessions, get().activeSessionId ?? sessionId));
    return get().send("", sessionId, undefined, {
      assistantId,
      resumeConfirmed: choice === "once" || choice === "session",
    });
  },

  send: async (text, targetSessionId, attachments, replay) => {
    const trimmed = replay ? "" : text.trim();
    const pending = attachments ?? [];
    if (get().sending || get().shellRunning) return false;
    if (!replay && !trimmed && pending.length === 0) return false;
    if (!get().hydrated) return false;

    const selection = useSelectionStore.getState().selection;
    if (!selection || !isTauri()) return false;

    let sessions = get().sessions;
    let sessionId = targetSessionId ?? get().activeSessionId;
    if (targetSessionId) {
      if (!sessions.some((session) => session.id === targetSessionId)) return false;
    } else {
      const ensured = ensureSession(sessions, sessionId);
      if (ensured.activeSessionId !== get().activeSessionId) {
        set({ sessions: ensured.sessions, activeSessionId: ensured.activeSessionId });
        void persistSnapshot(snapshotFromState(ensured.sessions, ensured.activeSessionId));
      }
      sessions = ensured.sessions;
      sessionId = ensured.activeSessionId;
    }
    if (!sessionId) return false;
    const activeSession = sessions.find((session) => session.id === sessionId);
    if (!activeSession) return false;

    const stickActive = get().activeSessionId === sessionId || get().activeSessionId === null;
    const epoch = ++sendEpoch;
    set({
      sending: true,
      sendingSessionId: sessionId,
      error: undefined,
      ...(stickActive ? { activeSessionId: sessionId } : {}),
    });

    const isFirstMessage = !replay && sessionMessages(activeSession).length === 0;
    const request = resolveSendRequest();
    let content = "";
    if (!replay) {
      try {
        content = await resolveUserMessageContent(trimmed);
      } catch (error) {
        set({
          sending: false,
          sendingSessionId: null,
          error: ipcErrorMessage(error),
        });
        get().flushQueued();
        return true;
      }
    }
    if (get().sendingSessionId !== sessionId) return true;
    const latest = get().sessions.find((session) => session.id === sessionId);
    if (!latest) {
      set({ sending: false, sendingSessionId: null });
      get().flushQueued();
      return true;
    }
    const now = Date.now();
    const withUser = replay
      ? get().sessions
      : sortSessions(
          patchActiveSession(get().sessions, sessionId, (session) => ({
            ...session,
            preview: content.trim() || pending[0]?.name || content,
            updatedAt: now,
            messages: [
              ...sessionMessages(session),
              {
                id: nextId(),
                role: "user" as const,
                content,
                ...(pending.length > 0 ? { attachments: pending } : {}),
              },
            ],
          })),
        );
    if (!replay) {
      set({
        sessions: withUser,
        ...(stickActive ? { activeSessionId: sessionId } : {}),
      });
      void persistSnapshot(snapshotFromState(withUser, get().activeSessionId ?? sessionId));
    }

    const applyTitle = (title: string): void => {
      const nextSessions = sortSessions(
        patchActiveSession(get().sessions, sessionId, (session) => ({
          ...session,
          title,
        })),
      );
      set({ sessions: nextSessions });
      const active = get().activeSessionId ?? sessionId;
      void persistSnapshot(snapshotFromState(nextSessions, active));
    };

    if (isFirstMessage && !activeSession.title) {
      if (trimmed) {
        void generateSessionTitle(trimmed).then(applyTitle);
      } else if (pending[0]?.name) {
        applyTitle(pending[0].name);
      }
    }

    let paintTimer = 0;
    const cancelPaint = (): void => {
      if (!paintTimer) return;
      window.clearTimeout(paintTimer);
      paintTimer = 0;
    };

    try {
      const assistantId = replay?.assistantId ?? nextId();
      let thinkingStartedAt: number | undefined;
      let thinkingEndedAt: number | undefined;
      const rounds: ToolRoundTrace[] = [];
      let activeRoundIndex = 0;
      // Boundary: any non-tool chunk after a tool starts a new round so
      // reasoning/tool/reasoning/tool chains render as separate collapsibles.
      let lastChunkKind: ChatChunkKind | null = null;
      let roundStartedAt: number | undefined;
      const contentParts: string[] = [];
      const reasoningParts: string[] = [];
      let lastPaintAt = 0;
      const commitBuffer = (): void => {
        const round = rounds[activeRoundIndex];
        if (!round) return;
        if (contentParts.length > 0) {
          round.content = `${round.content ?? ""}${contentParts.join("")}`;
          contentParts.length = 0;
        }
        if (reasoningParts.length > 0) {
          round.reasoning = `${round.reasoning}${reasoningParts.join("")}`;
          reasoningParts.length = 0;
        }
      };
      const publishStreaming = (preview: string | null): void => {
        if (get().sendingSessionId !== sessionId) return;
        const nextSessions = get().sessions.map((session) => {
          if (session.id !== sessionId) return session;
          const index = session.messages.findIndex((message) => message.id === assistantId);
          if (index < 0) {
            return {
              ...session,
              preview: preview ?? session.preview,
              messages: [
                ...session.messages,
                {
                  id: assistantId,
                  role: "assistant" as const,
                  content: preview ?? "",
                  toolRounds: snapshotRounds(),
                  streaming: true,
                },
              ],
            };
          }
          const messages = session.messages.slice();
          const current = messages[index];
          if (!current) return session;
          messages[index] = {
            ...current,
            content: preview ?? current.content,
            toolRounds: snapshotRounds(),
            streaming: true,
          };
          return {
            ...session,
            preview: preview ?? session.preview,
            messages,
          };
        });
        set({ sessions: nextSessions });
      };
      const paintStreaming = (): void => {
        paintTimer = 0;
        commitBuffer();
        lastPaintAt = performance.now();
        publishStreaming(rounds[activeRoundIndex]?.content ?? null);
      };
      const schedulePaint = (): void => {
        const now = performance.now();
        const sincePaint = lastPaintAt === 0 ? STREAM_PAINT_MS : now - lastPaintAt;
        if (sincePaint >= STREAM_PAINT_MS) {
          cancelPaint();
          paintStreaming();
          return;
        }
        if (paintTimer) return;
        paintTimer = window.setTimeout(paintStreaming, STREAM_PAINT_MS - sincePaint);
      };
      const ensureActiveRound = (): ToolRoundTrace => {
        while (rounds.length <= activeRoundIndex) {
          rounds.push({ reasoning: "", calls: [] });
        }
        return rounds[activeRoundIndex];
      };
      const recordRoundThinkingMs = (round: ToolRoundTrace, endedAt: number): void => {
        if (round.thinkingMs === undefined && roundStartedAt !== undefined) {
          round.thinkingMs = Math.max(0, endedAt - roundStartedAt);
        }
      };
      const snapshotRounds = (): ToolRoundTrace[] =>
        rounds.map((round) => {
          const snap: ToolRoundTrace = {
            reasoning: round.reasoning,
            calls: [...round.calls],
          };
          if (round.content !== undefined) snap.content = round.content;
          if (round.thinkingMs !== undefined) snap.thinkingMs = round.thinkingMs;
          return snap;
        });
      const onChunk = new Channel<ChatChunk>();
      onChunk.onmessage = (chunk) => {
        if (!chunk.text || get().sendingSessionId !== sessionId) return;
        if (chunk.kind === "tool_result") {
          const update = parseToolResultChunk(chunk.text);
          if (update) {
            let matched = false;
            for (const round of rounds) {
              const index = round.calls.findIndex((call) => call.id && call.id === update.id);
              if (index < 0) continue;
              const current = round.calls[index];
              if (!current) continue;
              const calls = round.calls.slice();
              calls[index] = {
                ...current,
                output: update.output ?? current.output,
                display: update.display ?? current.display,
              };
              round.calls = calls;
              matched = true;
              break;
            }
            if (!matched) {
              const round = ensureActiveRound();
              round.calls = [...round.calls, update];
            }
          }
          cancelPaint();
          commitBuffer();
          publishStreaming(rounds[activeRoundIndex]?.content ?? null);
          return;
        }
        const isReasoning = chunk.kind === "reasoning";
        const isTool = chunk.kind === "tool";
        if (isReasoning || isTool) {
          thinkingStartedAt ??= Date.now();
        } else if (thinkingStartedAt !== undefined && thinkingEndedAt === undefined) {
          thinkingEndedAt = Date.now();
        }
        if (chunk.kind === "question") {
          cancelPaint();
          commitBuffer();
          const pending = handleAskUserChunk(chunk.text, assistantId, sessionId);
          if (pending) {
            const activeRound = ensureActiveRound();
            const calls = activeRound.calls.map((call) => ({ ...call }));
            let index = -1;
            for (let cursor = calls.length - 1; cursor >= 0; cursor -= 1) {
              if (calls[cursor]?.name === "ask_user") {
                index = cursor;
                break;
              }
            }
            const current = index >= 0 ? calls[index] : undefined;
            if (current && index >= 0) {
              calls[index] = {
                ...current,
                id: pending.callId,
                arguments: pending.arguments || current.arguments,
                thoughtSignature: pending.thoughtSignature || current.thoughtSignature,
              };
              activeRound.calls = calls;
            }
          }
          const nextSessions = get().sessions.map((session) => {
            if (session.id !== sessionId) return session;
            const index = session.messages.findIndex((message) => message.id === assistantId);
            const pendingAsk = pending
              ? { callId: pending.callId, questions: pending.questions }
              : undefined;
            if (index < 0) {
              return {
                ...session,
                messages: [
                  ...session.messages,
                  {
                    id: assistantId,
                    role: "assistant" as const,
                    content: "",
                    toolRounds: snapshotRounds(),
                    streaming: true,
                    ...(pendingAsk ? { pendingAsk } : {}),
                  },
                ],
              };
            }
            const messages = session.messages.slice();
            const current = messages[index];
            if (!current) return session;
            messages[index] = {
              ...current,
              toolRounds: snapshotRounds(),
              streaming: true,
              ...(pendingAsk ? { pendingAsk } : {}),
            };
            return { ...session, messages };
          });
          set({ sessions: nextSessions });
          void persistSnapshot(snapshotFromState(nextSessions, get().activeSessionId ?? sessionId));
          return;
        }
        if (chunk.kind === "todo") {
          cancelPaint();
          commitBuffer();
          const parsed = parseTodoChunk(chunk.text);
          if (parsed) {
            const { todos } = parsed;
            const nextSessions = get().sessions.map((session) => {
              if (session.id !== sessionId) return session;
              const index = session.messages.findIndex((message) => message.id === assistantId);
              if (index < 0) return session;
              const messages = session.messages.slice();
              const current = messages[index];
              if (!current) return session;
              messages[index] = { ...current, todos };
              return { ...session, messages, todos };
            });
            set({ sessions: nextSessions });
            void persistSnapshot(
              snapshotFromState(nextSessions, get().activeSessionId ?? sessionId),
            );
            return;
          }
        }
        if (lastChunkKind === "tool" && !isTool) {
          commitBuffer();
          activeRoundIndex += 1;
          roundStartedAt = undefined;
        }
        const activeRound = ensureActiveRound();
        if (isReasoning) {
          if (roundStartedAt === undefined) roundStartedAt = Date.now();
          reasoningParts.push(chunk.text);
        } else if (isTool) {
          recordRoundThinkingMs(activeRound, Date.now());
          roundStartedAt = undefined;
          const toolCall = parseToolChunkText(chunk.text);
          if (toolCall) activeRound.calls = [...activeRound.calls, toolCall];
        } else if (chunk.kind === "content") {
          contentParts.push(chunk.text);
        }
        lastChunkKind = chunk.kind;
        if (!isTool) {
          schedulePaint();
          return;
        }
        cancelPaint();
        commitBuffer();
        publishStreaming(activeRound.content ?? null);
      };

      const { forceResponseLanguage, responseLanguage, reminderInterval } =
        useSettingsStore.getState();
      const selectedAgent = useComposerStore.getState().selectedAgent;
      const agentContexts = useAgentsStore.getState().contexts;
      const skillContexts = useSkillsStore.getState().contexts;
      const t = i18n.t.bind(i18n);
      const agent = resolveAgentMeta(selectedAgent, agentContexts, t);
      let historyMessages = sessionMessages(withUser.find((session) => session.id === sessionId));
      const loadedSkills = loadedSkillNamesFromMessages(historyMessages);
      const baseSystem = composeAgentSystem(agent, skillContexts, loadedSkills);
      const rules = buildAgentsMdRules(useAgentsMdStore.getState().files);
      const system = composeSystemWithLanguage(
        baseSystem,
        forceResponseLanguage,
        responseLanguage,
        rules,
      );
      const toolNames = agent?.tools ?? [];
      if (!replay) {
        historyMessages = await compactHistory(
          sessionId,
          historyMessages,
          system,
          toolNames,
          selection,
        );
      }
      const chatTurns = applySystemReminder(toChatTurns(historyMessages), system, reminderInterval);
      const result = await invoke<SendChatResult>("send_chat_message", {
        input: {
          providerId: selection.providerId,
          modelId: selection.modelId,
          messages: chatTurns,
          system: system.length > 0 ? system : null,
          request,
          sessionId: sessionId,
          outsideWorkspaceAllowed: Boolean(
            get().sessions.find((session) => session.id === sessionId)?.outsideWorkspaceAllowed,
          ),
          httpWriteAllowed: Boolean(
            get().sessions.find((session) => session.id === sessionId)?.httpWriteAllowed,
          ),
          resumeConfirmed: Boolean(replay?.resumeConfirmed),
          toolNames,
          workerCores: getWorkerCoreSnapshot().limit,
          allowedCommands: useSettingsStore.getState().allowedCommands,
          blockedCommands: useSettingsStore.getState().blockedCommands,
          shellProgram: useSettingsStore.getState().shellProgram,
        },
        onChunk,
      });
      cancelPaint();
      if (epoch !== sendEpoch) return true;
      const replyAt = Date.now();
      const duration = thinkingDurationMs(thinkingStartedAt, thinkingEndedAt ?? replyAt);
      const withAssistant = get().sessions.map((session) => {
        if (session.id !== sessionId) return session;
        const index = session.messages.findIndex((message) => message.id === assistantId);
        if (index < 0) {
          return {
            ...session,
            preview: result.content,
            updatedAt: replyAt,
            messages: [
              ...session.messages,
              {
                id: assistantId,
                role: "assistant" as const,
                content: result.content,
                reasoning: result.reasoning,
                reasoningSignature: result.reasoningSignature,
                thinkingMs: duration,
                toolRounds: result.toolRounds,
              },
            ],
          };
        }
        const messages = session.messages.slice();
        const current = messages[index];
        if (!current) return session;
        messages[index] = {
          ...withoutAskFlags(current),
          content: result.content || current.content,
          reasoning: result.reasoning,
          reasoningSignature: result.reasoningSignature || current.reasoningSignature,
          thinkingMs: duration ?? current.thinkingMs,
          toolCalls: undefined,
          toolRounds: result.toolRounds?.length ? result.toolRounds : current.toolRounds,
          streaming: false,
        };
        return {
          ...session,
          preview: messages[index]?.content ?? result.content,
          updatedAt: replyAt,
          messages,
        };
      });
      const stillSending = get().sendingSessionId === sessionId;
      set({
        sessions: withAssistant,
        ...(stillSending ? { sending: false, sendingSessionId: null } : {}),
      });
      const active = get().activeSessionId ?? sessionId;
      void persistSnapshot(snapshotFromState(withAssistant, active));
      if (stillSending) {
        const finished = withAssistant
          .find((session) => session.id === sessionId)
          ?.messages.find((message) => message.id === assistantId);
        if (finished && !finished.interrupted) {
          void notifyResponseFinished(finished.content);
        }
        get().flushQueued();
      }
      return true;
    } catch (error) {
      cancelPaint();
      if (epoch !== sendEpoch) return true;
      if (get().sendingSessionId === sessionId) {
        const cancelled = ipcErrorMessage(error).toLowerCase().includes("interrupted by user");
        const nextSessions = get().sessions.map((session) => {
          if (session.id !== sessionId) return session;
          return {
            ...session,
            messages: session.messages.map((message) =>
              message.streaming
                ? {
                    ...withoutAskFlags(message),
                    streaming: false,
                    interrupted: cancelled || message.interrupted,
                  }
                : message,
            ),
          };
        });
        set({
          sessions: nextSessions,
          sending: false,
          sendingSessionId: null,
          interruptArmedAt: null,
          error: cancelled ? undefined : ipcErrorMessage(error),
        });
        void persistSnapshot(snapshotFromState(nextSessions, sessionId));
        get().flushQueued();
      }
      return true;
    }
  },

  enqueue: (text, mode, attachments) => {
    const trimmed = text.trim();
    const pending = attachments ?? [];
    if ((!trimmed && (mode === "shell" || pending.length === 0)) || !get().hydrated) return;
    const ensured = ensureSession(get().sessions, get().activeSessionId);
    if (ensured.activeSessionId !== get().activeSessionId) {
      set({ sessions: ensured.sessions, activeSessionId: ensured.activeSessionId });
      void persistSnapshot(snapshotFromState(ensured.sessions, ensured.activeSessionId));
    }
    const sessionId = ensured.activeSessionId;
    if (!sessionId) return;
    set({
      queued: [
        ...get().queued,
        {
          id: nextId(),
          sessionId,
          text: trimmed,
          mode,
          ...(pending.length > 0 ? { attachments: pending } : {}),
        },
      ],
    });
    get().flushQueued();
  },

  removeQueued: (id) => {
    set({ queued: get().queued.filter((item) => item.id !== id) });
  },

  editQueued: (id) => {
    const item = get().queued.find((entry) => entry.id === id);
    if (!item) return;
    const composer = useComposerStore.getState();
    composer.setValue(item.text);
    if (item.attachments && item.attachments.length > 0) {
      composer.setAttachments(item.attachments);
    }
    if (composer.mode !== item.mode) {
      composer.setMode(item.mode);
    }
    set({ queued: get().queued.filter((entry) => entry.id !== id) });
  },

  flushQueued: () => {
    if (get().sending || get().shellRunning) return;
    const next = get().queued[0];
    if (!next) return;
    set({ queued: get().queued.slice(1) });
    const started =
      next.mode === "shell"
        ? get().runShell(next.text, next.sessionId)
        : get().send(next.text, next.sessionId, next.attachments);
    void started.then(
      (ok) => {
        if (!ok) get().flushQueued();
      },
      (error: unknown) => {
        console.warn("queued task failed", error);
        if (!get().sending && !get().shellRunning) get().flushQueued();
      },
    );
  },

  sendQueuedNow: async (id) => {
    if (get().queued.length === 0) return;
    if (id) {
      const current = get().queued;
      const item = current.find((entry) => entry.id === id);
      if (!item) return;
      set({ queued: [item, ...current.filter((entry) => entry.id !== id)] });
    }
    if (get().sending || get().shellRunning) {
      await get().interruptActiveTask();
    }
    get().flushQueued();
  },

  interruptActiveTask: async () => {
    const state = get();
    const sessionId = state.sendingSessionId ?? state.shellRunningSessionId;
    if (!sessionId) return false;
    try {
      await invoke<boolean>("cancel_running_task", { sessionId });
    } catch (error) {
      console.warn("cancel_running_task failed", error);
    }
    set({ interruptArmedAt: null });
    return true;
  },

  armInterrupt: () => {
    set({ interruptArmedAt: Date.now() });
  },

  disarmInterrupt: () => {
    if (get().interruptArmedAt !== null) set({ interruptArmedAt: null });
  },

  rewindTo: (messageId) => {
    const { sessions, activeSessionId, sendingSessionId, shellRunningSessionId } = get();
    const sessionId = activeSessionId;
    if (!sessionId) return;
    if (sendingSessionId === sessionId) {
      console.warn("[sessions] rewind blocked: send in progress");
      return;
    }
    if (shellRunningSessionId === sessionId) {
      console.warn("[sessions] rewind blocked: shell in progress");
      return;
    }
    const session = sessions.find((item) => item.id === sessionId);
    if (!session) return;
    const messages = sessionMessages(session);
    const index = messages.findIndex((message) => message.id === messageId);
    if (index < 0) return;
    const removed = messages.length - index;

    // TODO: rewind file edits and shell changes made by the assistant
    // messages between `index` and the previous end. Track per-write snapshots
    // when the assistant calls edit tools (write_file, edit_file, run_shell)
    // and replay restoration here, in reverse order, after the chat trim.
    // For now we only roll back the chat history.
    console.warn(`[sessions] rewind to ${messageId}: trimmed ${removed} message(s), kept ${index}`);

    const kept = messages.slice(0, index);
    const nextSessions = sortSessions(
      patchActiveSession(sessions, sessionId, (item) => ({
        ...item,
        messages: kept,
        preview: previewFromMessages(kept),
        updatedAt: Date.now(),
      })),
    );
    set({ sessions: nextSessions, error: undefined });
    void persistSnapshot(snapshotFromState(nextSessions, sessionId));
  },

  runShell: async (text, targetSessionId) => {
    const trimmed = text.trim();
    if (!trimmed || get().shellRunning || get().sending) return false;
    if (!get().hydrated) return false;

    let sessions = get().sessions;
    let sessionId = targetSessionId ?? get().activeSessionId;
    if (targetSessionId) {
      if (!sessions.some((session) => session.id === targetSessionId)) return false;
    } else {
      const ensured = ensureSession(sessions, sessionId);
      if (ensured.activeSessionId !== get().activeSessionId) {
        set({ sessions: ensured.sessions, activeSessionId: ensured.activeSessionId });
        void persistSnapshot(snapshotFromState(ensured.sessions, ensured.activeSessionId));
      }
      sessions = ensured.sessions;
      sessionId = ensured.activeSessionId;
    }
    if (!sessionId) return false;
    if (!sessions.some((session) => session.id === sessionId)) return false;

    const stickActive = get().activeSessionId === sessionId || get().activeSessionId === null;
    set({
      shellRunning: true,
      shellRunningSessionId: sessionId,
      error: undefined,
      ...(stickActive ? { activeSessionId: sessionId } : {}),
    });

    const workspaceRoot = await invoke<string | null>("get_workspace_path").catch(() => null);
    if (!workspaceRoot) {
      set({
        shellRunning: false,
        shellRunningSessionId: null,
        error: i18n.t("chat.shell.needsWorkspace"),
      });
      get().flushQueued();
      return true;
    }

    if (get().shellRunningSessionId !== sessionId) return true;
    let resolvedCommand: string;
    try {
      resolvedCommand = await resolveOutgoingMentions(trimmed, workspaceRoot, "shell");
    } catch (error) {
      set({
        shellRunning: false,
        shellRunningSessionId: null,
        error: ipcErrorMessage(error),
      });
      get().flushQueued();
      return true;
    }
    if (get().shellRunningSessionId !== sessionId) return true;
    const latest = get().sessions.find((session) => session.id === sessionId);
    if (!latest) {
      set({ shellRunning: false, shellRunningSessionId: null });
      get().flushQueued();
      return true;
    }

    const now = Date.now();
    const isFirstMessage = sessionMessages(latest).length === 0;
    const userMessageId = nextId();
    const userMessage: ChatMessage = {
      id: userMessageId,
      role: "user",
      kind: "shell",
      content: formatShellMessage(resolvedCommand, i18n.t("chat.shell.running")),
    };
    const withUser = sortSessions(
      patchActiveSession(get().sessions, sessionId, (session) => ({
        ...session,
        preview: resolvedCommand,
        updatedAt: now,
        messages: [...sessionMessages(session), userMessage],
      })),
    );
    set({
      sessions: withUser,
      ...(stickActive ? { activeSessionId: sessionId } : {}),
    });
    void persistSnapshot(snapshotFromState(withUser, get().activeSessionId ?? sessionId));

    if (isFirstMessage && !latest.title) {
      void generateSessionTitle(trimmed).then((title) => {
        const nextSessions = sortSessions(
          patchActiveSession(get().sessions, sessionId, (session) => ({
            ...session,
            title,
          })),
        );
        set({ sessions: nextSessions });
        const active = get().activeSessionId ?? sessionId;
        void persistSnapshot(snapshotFromState(nextSessions, active));
      });
    }

    const streamBuffer: { stdout: string; stderr: string } = { stdout: "", stderr: "" };

    const displayStream = (): string => {
      const parts: string[] = [];
      if (streamBuffer.stdout.length > 0) parts.push(streamBuffer.stdout.replace(/\n$/, ""));
      if (streamBuffer.stderr.length > 0) {
        parts.push(`stderr:\n${streamBuffer.stderr.replace(/\n$/, "")}`);
      }
      if (parts.length === 0) return i18n.t("chat.shell.running");
      return parts.join("\n\n");
    };

    const applyShellStream = (): void => {
      const content = formatShellMessage(resolvedCommand, displayStream());
      const nextSessions = patchActiveSession(get().sessions, sessionId, (session) => {
        const messages = session.messages.slice();
        const idx = messages.findIndex((message) => message.id === userMessageId);
        if (idx < 0) return session;
        const current = messages[idx];
        if (!current) return session;
        messages[idx] = { ...current, content, streaming: true };
        return { ...session, preview: resolvedCommand, messages };
      });
      set({ sessions: nextSessions });
    };

    let streamFrame = 0;
    const cancelStreamFrame = (): void => {
      if (streamFrame === 0) return;
      cancelAnimationFrame(streamFrame);
      streamFrame = 0;
    };
    const scheduleShellStream = (): void => {
      if (streamFrame !== 0) return;
      streamFrame = requestAnimationFrame(() => {
        streamFrame = 0;
        if (get().shellRunningSessionId !== sessionId) return;
        applyShellStream();
      });
    };

    const applyShellMessage = (content: string, shellAiSummary?: string, error?: string): void => {
      const nextSessions = sortSessions(
        patchActiveSession(get().sessions, sessionId, (session) => {
          const messages = session.messages.slice();
          const idx = messages.findIndex((message) => message.id === userMessageId);
          if (idx >= 0) {
            const current = messages[idx];
            if (!current) return session;
            messages[idx] = {
              ...current,
              content,
              shellAiSummary,
              streaming: false,
            };
          }
          return {
            ...session,
            preview: content,
            updatedAt: Date.now(),
            messages,
          };
        }),
      );
      const stillRunning = get().shellRunningSessionId === sessionId;
      set({
        sessions: nextSessions,
        ...(stillRunning ? { shellRunning: false, shellRunningSessionId: null } : {}),
        ...(stillRunning ? { interruptArmedAt: null } : {}),
        error,
      });
      const active = get().activeSessionId ?? sessionId;
      void persistSnapshot(snapshotFromState(nextSessions, active));
      if (stillRunning) get().flushQueued();
    };

    const onChunk = new Channel<ShellChunk>();
    onChunk.onmessage = (chunk) => {
      if (get().shellRunningSessionId !== sessionId) return;
      if (chunk.kind === "stdout") streamBuffer.stdout += chunk.text;
      else if (chunk.kind === "stderr") streamBuffer.stderr += chunk.text;
      scheduleShellStream();
    };

    try {
      const result = await runShellCommand(
        {
          command: resolvedCommand,
          sessionId,
          shell: useSettingsStore.getState().shellProgram || undefined,
        },
        onChunk,
      );
      cancelStreamFrame();
      const output = buildShellResultContent(result);
      const aiOutput = summarizeShellResultForAi(result);
      const finalOutput = result.cancelled ? appendInterruptedFooter(output) : output;
      const finalAiOutput = result.cancelled ? appendInterruptedFooter(aiOutput) : aiOutput;
      const content = formatShellMessage(resolvedCommand, finalOutput);
      applyShellMessage(
        content,
        finalAiOutput !== finalOutput
          ? formatShellMessage(resolvedCommand, finalAiOutput)
          : undefined,
      );
    } catch (error) {
      cancelStreamFrame();
      const errorMessage = ipcErrorMessage(error);
      const failed = i18n.t("chat.shell.failed", { error: errorMessage });
      applyShellMessage(formatShellMessage(resolvedCommand, failed), failed, errorMessage);
    }
    return true;
  },

  rewindLastUserMessage: () => {
    const { sessions, activeSessionId } = get();
    const sessionId = activeSessionId;
    if (!sessionId) return;
    const session = sessions.find((item) => item.id === sessionId);
    if (!session) return;
    let lastUserId: string | null = null;
    const messages = sessionMessages(session);
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      const message = messages[i];
      if (message?.role === "user") {
        lastUserId = message.id;
        break;
      }
    }
    if (!lastUserId) return;
    get().rewindTo(lastUserId);
  },
}));

export const selectActiveSession = (state: SessionsStore): SessionRecord | null => {
  const { sessions, activeSessionId } = state;
  if (!activeSessionId) return null;
  return sessions.find((session) => session.id === activeSessionId) ?? null;
};

const EMPTY_MESSAGES: ChatMessage[] = [];

export const selectActiveMessages = (state: SessionsStore): ChatMessage[] => {
  const messages = sessionMessages(selectActiveSession(state));
  return messages.length > 0 ? messages : EMPTY_MESSAGES;
};
