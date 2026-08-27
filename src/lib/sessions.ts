import { create } from "zustand";
import { Channel, invoke } from "@tauri-apps/api/core";
import i18n from "@/i18n";
import { buildAgentsMdRules, composeAgentSystem } from "@/lib/agent-system";
import { resolveAgentMeta } from "@/lib/builtin-agents";
import { useAgentsMdStore } from "@/lib/agents-md";
import { useAgentsStore } from "@/lib/agents";
import { useComposerStore, type ComposerMode } from "@/lib/composer";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { useProvidersStore } from "@/lib/providers";
import { resolveOutgoingMentions } from "@/lib/resolve-outgoing-mentions";
import { notifyResponseFinished } from "@/lib/notifications";
import { composeSystemWithLanguage } from "@/lib/response-language";
import { selectEffort, useSelectionStore } from "@/lib/selected-model";
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
import { loadedSkillNamesFromMessages } from "@/lib/context-usage";
import { sanitizeSessionsSnapshot, sessionMessages, toChatTurns } from "@/lib/session-turns";
import { getWorkerCoreSnapshot } from "@/lib/worker-cores";
import {
  parseToolChunkText,
  type AskUserQuestion,
  type AskUserQuestionChunk,
  type ChatAttachment,
  type ChatChunk,
  type ChatChunkKind,
  type ChatMessage,
  type SelectedModel,
  type SendChatResult,
  type ToolRoundTrace,
} from "@/types/chat";
import { useAskUserStore } from "@/lib/ask-user";
import { notifyAskUser } from "@/lib/notifications";
import {
  sortSessions,
  titleFromFirstMessage,
  type SessionRecord,
  type SessionsSnapshot,
} from "@/types/sessions";

export const INTERRUPT_ARM_MS = 2000;

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

const handleAskUserChunk = (raw: string, messageId: string): void => {
  try {
    const parsed = JSON.parse(raw) as Partial<AskUserQuestionChunk>;
    if (!parsed.callId || !Array.isArray(parsed.questions)) return;
    const questions = parsed.questions.filter(
      (question): question is AskUserQuestion =>
        Boolean(question) &&
        typeof question.id === "string" &&
        typeof question.header === "string" &&
        typeof question.question === "string" &&
        Array.isArray(question.options),
    );
    if (questions.length === 0) return;
    useAskUserStore.getState().upsert({
      callId: parsed.callId,
      messageId,
      questions,
      answers: questions.map((question) => ({
        questionId: question.id,
        selected: [],
        freeText: "",
      })),
    });
    const first = questions[0];
    void notifyAskUser(questions.length, first ? first.question : null);
  } catch (error) {
    console.warn("ask_user chunk parse failed", error);
  }
};

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

const emptySession = (): SessionRecord => ({
  id: nextId(),
  title: "",
  preview: "",
  updatedAt: Date.now(),
  messages: [],
});

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
      .filter((message) => !message.streaming)
      .map((message) => {
        const { toolCalls: _toolCalls, streaming: _streaming, ...rest } = message;
        return rest;
      }),
  })),
});

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

const resolveSendEffort = (selection: SelectedModel): string | undefined => {
  const stored = selectEffort(useSelectionStore.getState());
  if (stored) return stored;
  const provider = useProvidersStore
    .getState()
    .providers.find((item) => item.id === selection.providerId);
  const model = provider?.models.find((item) => item.id === selection.modelId);
  return model?.effortLevels?.[0];
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
  create: () => void;
  select: (id: string) => void;
  remove: (id: string) => void;
  send: (text: string, sessionId?: string, attachments?: ChatAttachment[]) => Promise<boolean>;
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
    try {
      const snapshot = sanitizeSessionsSnapshot(
        await invokeWithTimeout<SessionsSnapshot>("load_sessions", HYDRATE_TIMEOUT_MS),
      );
      if (get().hydrated) return;
      const ensured = ensureSession(snapshot.sessions, snapshot.activeSessionId);
      set({
        sessions: ensured.sessions,
        activeSessionId: ensured.activeSessionId,
        hydrated: true,
      });
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

  create: () => {
    const { sessions, activeSessionId } = get();
    const current = activeSessionId
      ? sessions.find((session) => session.id === activeSessionId)
      : undefined;
    if (current && isBlankSession(current)) {
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

  remove: (id) => {
    const { sessions, activeSessionId, sendingSessionId, shellRunningSessionId } = get();
    const nextSessions = sessions.filter((session) => session.id !== id);
    const stopSending = sendingSessionId === id;
    const stopShell = shellRunningSessionId === id;
    if (nextSessions.length === 0) {
      const session = emptySession();
      const seeded = [session];
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
    const fallbackId = sorted[0]?.id;
    if (!fallbackId) return;
    const nextActiveId =
      activeSessionId !== null &&
      activeSessionId !== id &&
      sorted.some((session) => session.id === activeSessionId)
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

  send: async (text, targetSessionId, attachments) => {
    const trimmed = text.trim();
    const pending = attachments ?? [];
    if ((!trimmed && pending.length === 0) || get().sending || get().shellRunning) return false;
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
    set({
      sending: true,
      sendingSessionId: sessionId,
      error: undefined,
      ...(stickActive ? { activeSessionId: sessionId } : {}),
    });

    const isFirstMessage = sessionMessages(activeSession).length === 0;
    const effort = resolveSendEffort(selection);
    let content: string;
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
    if (get().sendingSessionId !== sessionId) return true;
    const latest = get().sessions.find((session) => session.id === sessionId);
    if (!latest) {
      set({ sending: false, sendingSessionId: null });
      get().flushQueued();
      return true;
    }
    const userMessage: ChatMessage = {
      id: nextId(),
      role: "user",
      content,
      ...(pending.length > 0 ? { attachments: pending } : {}),
    };
    const now = Date.now();
    const withUser = sortSessions(
      patchActiveSession(get().sessions, sessionId, (session) => ({
        ...session,
        preview: content.trim() || pending[0]?.name || content,
        updatedAt: now,
        messages: [...sessionMessages(session), userMessage],
      })),
    );
    set({
      sessions: withUser,
      ...(stickActive ? { activeSessionId: sessionId } : {}),
    });
    void persistSnapshot(snapshotFromState(withUser, get().activeSessionId ?? sessionId));

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

    try {
      const assistantId = nextId();
      const chatTurns = toChatTurns(
        sessionMessages(withUser.find((session) => session.id === sessionId)),
      );
      let thinkingStartedAt: number | undefined;
      let thinkingEndedAt: number | undefined;
      const rounds: ToolRoundTrace[] = [];
      let activeRoundIndex = 0;
      // Boundary: any non-tool chunk after a tool starts a new round so
      // reasoning/tool/reasoning/tool chains render as separate collapsibles.
      let lastChunkKind: ChatChunkKind | null = null;
      let roundStartedAt: number | undefined;
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
        const isReasoning = chunk.kind === "reasoning";
        const isTool = chunk.kind === "tool";
        if (isReasoning || isTool) {
          thinkingStartedAt ??= Date.now();
        } else if (thinkingStartedAt !== undefined && thinkingEndedAt === undefined) {
          thinkingEndedAt = Date.now();
        }
        if (chunk.kind === "question") {
          handleAskUserChunk(chunk.text, assistantId);
          return;
        }
        if (lastChunkKind === "tool" && !isTool) {
          activeRoundIndex += 1;
          roundStartedAt = undefined;
        }
        const activeRound = ensureActiveRound();
        if (isReasoning) {
          if (roundStartedAt === undefined) roundStartedAt = Date.now();
          activeRound.reasoning = `${activeRound.reasoning}${chunk.text}`;
        } else if (isTool) {
          recordRoundThinkingMs(activeRound, Date.now());
          roundStartedAt = undefined;
          const toolCall = parseToolChunkText(chunk.text);
          if (toolCall) activeRound.calls = [...activeRound.calls, toolCall];
        } else if (chunk.kind === "content") {
          activeRound.content = `${activeRound.content ?? ""}${chunk.text}`;
        }
        lastChunkKind = chunk.kind;
        const newPreview = chunk.kind === "content" ? (activeRound.content ?? chunk.text) : null;
        const nextSessions = get().sessions.map((session) => {
          if (session.id !== sessionId) return session;
          const index = session.messages.findIndex((message) => message.id === assistantId);
          if (index < 0) {
            return {
              ...session,
              preview: newPreview ?? session.preview,
              messages: [
                ...session.messages,
                {
                  id: assistantId,
                  role: "assistant" as const,
                  content: chunk.kind === "content" ? chunk.text : "",
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
            content:
              chunk.kind === "content" ? (activeRound.content ?? chunk.text) : current.content,
            toolRounds: snapshotRounds(),
            streaming: true,
          };
          return {
            ...session,
            preview: newPreview ?? session.preview,
            messages,
          };
        });
        set({ sessions: nextSessions });
      };

      const { forceResponseLanguage, responseLanguage } = useSettingsStore.getState();
      const selectedAgent = useComposerStore.getState().selectedAgent;
      const agentContexts = useAgentsStore.getState().contexts;
      const skillContexts = useSkillsStore.getState().contexts;
      const t = i18n.t.bind(i18n);
      const agent = resolveAgentMeta(selectedAgent, agentContexts, t);
      const historyMessages = sessionMessages(withUser.find((session) => session.id === sessionId));
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
      const result = await invoke<SendChatResult>("send_chat_message", {
        input: {
          providerId: selection.providerId,
          modelId: selection.modelId,
          messages: chatTurns,
          system: system.length > 0 ? system : null,
          effort: effort ?? null,
          sessionId: sessionId,
          toolNames,
          workerCores: getWorkerCoreSnapshot().limit,
        },
        onChunk,
      });
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
          ...current,
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
      if (get().sendingSessionId === sessionId) {
        const cancelled = ipcErrorMessage(error).toLowerCase().includes("interrupted by user");
        const nextSessions = get().sessions.map((session) => {
          if (session.id !== sessionId) return session;
          return {
            ...session,
            messages: session.messages.map((message) =>
              message.streaming
                ? { ...message, streaming: false, interrupted: cancelled || message.interrupted }
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
