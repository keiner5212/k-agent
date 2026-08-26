import { create } from "zustand";
import { Channel, invoke } from "@tauri-apps/api/core";
import i18n from "@/i18n";
import { useAgentsStore } from "@/lib/agents";
import { resolveAgentPersonality } from "@/lib/builtin-agents";
import { useComposerStore } from "@/lib/composer";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { useProvidersStore } from "@/lib/providers";
import { resolveOutgoingMentions } from "@/lib/resolve-outgoing-mentions";
import { composeSystemWithLanguage } from "@/lib/response-language";
import { selectEffort, useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
import {
  buildShellResultContent,
  formatShellMessage,
  runShellCommand,
  summarizeShellResultForAi,
  type ShellChunk,
} from "@/lib/shell";
import type { ChatChunk, ChatMessage, ChatTurn, SelectedModel, SendChatResult } from "@/types/chat";
import {
  sortSessions,
  titleFromFirstMessage,
  type SessionRecord,
  type SessionsSnapshot,
} from "@/types/sessions";

const toChatTurns = (messages: ChatMessage[]): ChatTurn[] =>
  messages
    .filter((message) => !message.streaming)
    .filter(
      (message) => message.content.trim().length > 0 || (message.reasoning ?? "").trim().length > 0,
    )
    .map((message) => ({
      role: message.role,
      content: message.shellAiSummary ?? message.content,
      reasoning: message.reasoning ?? null,
      reasoningSignature: message.reasoningSignature ?? null,
    }));

const previewFromMessages = (messages: ChatMessage[]): string => {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const content = messages[i]?.content.trim();
    if (content && content.length > 0) return content;
  }
  return "";
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

const persistSnapshot = async (snapshot: SessionsSnapshot): Promise<void> => {
  if (!isTauri()) return;
  try {
    await invoke("save_sessions", { snapshot });
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
  session.messages.length === 0 && session.title.trim().length === 0;

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
  error?: string;
  hydrate: () => Promise<void>;
  create: () => void;
  select: (id: string) => void;
  remove: (id: string) => void;
  send: (text: string) => Promise<void>;
  runShell: (text: string) => Promise<void>;
  rewindTo: (messageId: string) => void;
  rewindLastUserMessage: () => void;
};

export const useSessionsStore = create<SessionsStore>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  sendingSessionId: null,
  shellRunningSessionId: null,
  hydrated: false,
  sending: false,
  shellRunning: false,

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
      const snapshot = await invoke<SessionsSnapshot>("load_sessions");
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
        ...(stopSending ? { sending: false, sendingSessionId: null } : {}),
        ...(stopShell ? { shellRunning: false, shellRunningSessionId: null } : {}),
      });
      void persistSnapshot(snapshotFromState(seeded, session.id));
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
      ...(stopSending ? { sending: false, sendingSessionId: null } : {}),
      ...(stopShell ? { shellRunning: false, shellRunningSessionId: null } : {}),
    });
    void persistSnapshot(snapshotFromState(sorted, nextActiveId));
  },

  send: async (text) => {
    const trimmed = text.trim();
    if (!trimmed || get().sending || get().shellRunning) return;
    if (!get().hydrated) return;

    const selection = useSelectionStore.getState().selection;
    if (!selection || !isTauri()) return;

    const ensured = ensureSession(get().sessions, get().activeSessionId);
    if (ensured.activeSessionId !== get().activeSessionId) {
      set({ sessions: ensured.sessions, activeSessionId: ensured.activeSessionId });
      void persistSnapshot(snapshotFromState(ensured.sessions, ensured.activeSessionId));
    }

    const sessionId = ensured.activeSessionId;
    const activeSession = ensured.sessions.find((session) => session.id === sessionId);
    if (!activeSession) return;

    const isFirstMessage = activeSession.messages.length === 0;
    const effort = resolveSendEffort(selection);
    const content = await resolveUserMessageContent(trimmed);
    const userMessage: ChatMessage = {
      id: nextId(),
      role: "user",
      content,
    };
    const now = Date.now();

    const withUser = sortSessions(
      patchActiveSession(ensured.sessions, sessionId, (session) => ({
        ...session,
        preview: content,
        updatedAt: now,
        messages: [...session.messages, userMessage],
      })),
    );
    set({
      sessions: withUser,
      activeSessionId: sessionId,
      sending: true,
      sendingSessionId: sessionId,
      error: undefined,
    });
    void persistSnapshot(snapshotFromState(withUser, sessionId));

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
      void generateSessionTitle(trimmed).then(applyTitle);
    }

    try {
      const assistantId = nextId();
      const chatTurns = toChatTurns(
        withUser.find((session) => session.id === sessionId)?.messages ?? [],
      );
      let thinkingStartedAt: number | undefined;
      let thinkingEndedAt: number | undefined;
      const onChunk = new Channel<ChatChunk>();
      onChunk.onmessage = (chunk) => {
        if (!chunk.text || get().sendingSessionId !== sessionId) return;
        const isReasoning = chunk.kind === "reasoning";
        if (isReasoning) {
          thinkingStartedAt ??= Date.now();
        } else if (thinkingStartedAt !== undefined && thinkingEndedAt === undefined) {
          thinkingEndedAt = Date.now();
        }
        const nextSessions = get().sessions.map((session) => {
          if (session.id !== sessionId) return session;
          const index = session.messages.findIndex((message) => message.id === assistantId);
          if (index < 0) {
            return {
              ...session,
              preview: isReasoning ? session.preview : chunk.text,
              messages: [
                ...session.messages,
                {
                  id: assistantId,
                  role: "assistant" as const,
                  content: isReasoning ? "" : chunk.text,
                  reasoning: isReasoning ? chunk.text : undefined,
                  streaming: true,
                },
              ],
            };
          }
          const messages = session.messages.slice();
          const current = messages[index];
          if (!current) return session;
          const content = isReasoning ? current.content : `${current.content}${chunk.text}`;
          const reasoning = isReasoning
            ? `${current.reasoning ?? ""}${chunk.text}`
            : current.reasoning;
          messages[index] = { ...current, content, reasoning, streaming: true };
          return {
            ...session,
            preview: isReasoning ? session.preview : content,
            messages,
          };
        });
        set({ sessions: nextSessions });
      };

      const { forceResponseLanguage, responseLanguage } = useSettingsStore.getState();
      const baseSystem = resolveAgentPersonality(
        useComposerStore.getState().selectedAgent,
        useAgentsStore.getState().contexts,
        i18n.t.bind(i18n),
      );
      const system = composeSystemWithLanguage(baseSystem, forceResponseLanguage, responseLanguage);
      const result = await invoke<SendChatResult>("send_chat_message", {
        input: {
          providerId: selection.providerId,
          modelId: selection.modelId,
          messages: chatTurns,
          system: system.length > 0 ? system : null,
          effort: effort ?? null,
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
          reasoning: result.reasoning || current.reasoning,
          reasoningSignature: result.reasoningSignature || current.reasoningSignature,
          thinkingMs: duration ?? current.thinkingMs,
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
    } catch (error) {
      if (get().sendingSessionId === sessionId) {
        const nextSessions = get().sessions.map((session) => {
          if (session.id !== sessionId) return session;
          return {
            ...session,
            messages: session.messages.map((message) =>
              message.streaming ? { ...message, streaming: false } : message,
            ),
          };
        });
        set({
          sessions: nextSessions,
          sending: false,
          sendingSessionId: null,
          error: ipcErrorMessage(error),
        });
      }
    }
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
    const index = session.messages.findIndex((message) => message.id === messageId);
    if (index < 0) return;
    const removed = session.messages.length - index;

    // TODO: rewind file edits and shell changes made by the assistant
    // messages between `index` and the previous end. Track per-write snapshots
    // when the assistant calls edit tools (write_file, edit_file, run_shell)
    // and replay restoration here, in reverse order, after the chat trim.
    // For now we only roll back the chat history.
    console.warn(`[sessions] rewind to ${messageId}: trimmed ${removed} message(s), kept ${index}`);

    const kept = session.messages.slice(0, index);
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

  runShell: async (text) => {
    const trimmed = text.trim();
    if (!trimmed || get().shellRunning || get().sending) return;
    if (!get().hydrated) return;

    const ensured = ensureSession(get().sessions, get().activeSessionId);
    if (ensured.activeSessionId !== get().activeSessionId) {
      set({ sessions: ensured.sessions, activeSessionId: ensured.activeSessionId });
      void persistSnapshot(snapshotFromState(ensured.sessions, ensured.activeSessionId));
    }
    const sessionId = ensured.activeSessionId;
    if (!sessionId) return;

    const workspaceRoot = await invoke<string | null>("get_workspace_path").catch(() => null);
    if (!workspaceRoot) {
      set({ error: i18n.t("chat.shell.needsWorkspace") });
      return;
    }

    const resolvedCommand = await resolveOutgoingMentions(trimmed, workspaceRoot, "shell");

    const now = Date.now();
    const activeSession = ensured.sessions.find((session) => session.id === sessionId);
    const isFirstMessage = (activeSession?.messages.length ?? 0) === 0;
    const userMessageId = nextId();
    const userMessage: ChatMessage = {
      id: userMessageId,
      role: "user",
      kind: "shell",
      content: formatShellMessage(resolvedCommand, i18n.t("chat.shell.running")),
    };
    const withUser = sortSessions(
      patchActiveSession(ensured.sessions, sessionId, (session) => ({
        ...session,
        preview: resolvedCommand,
        updatedAt: now,
        messages: [...session.messages, userMessage],
      })),
    );
    set({
      sessions: withUser,
      activeSessionId: sessionId,
      shellRunning: true,
      shellRunningSessionId: sessionId,
      error: undefined,
    });
    void persistSnapshot(snapshotFromState(withUser, sessionId));

    if (isFirstMessage && !activeSession?.title) {
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
        error,
      });
      const active = get().activeSessionId ?? sessionId;
      void persistSnapshot(snapshotFromState(nextSessions, active));
    };

    const onChunk = new Channel<ShellChunk>();
    onChunk.onmessage = (chunk) => {
      if (get().shellRunningSessionId !== sessionId) return;
      if (chunk.kind === "stdout") streamBuffer.stdout += chunk.text;
      else if (chunk.kind === "stderr") streamBuffer.stderr += chunk.text;
      scheduleShellStream();
    };

    try {
      const result = await runShellCommand({ command: resolvedCommand }, onChunk);
      cancelStreamFrame();
      const output = buildShellResultContent(result);
      const aiOutput = summarizeShellResultForAi(result);
      const content = formatShellMessage(resolvedCommand, output);
      applyShellMessage(
        content,
        aiOutput !== output ? formatShellMessage(resolvedCommand, aiOutput) : undefined,
      );
    } catch (error) {
      cancelStreamFrame();
      const errorMessage = ipcErrorMessage(error);
      const failed = i18n.t("chat.shell.failed", { error: errorMessage });
      applyShellMessage(formatShellMessage(resolvedCommand, failed), failed, errorMessage);
    }
  },

  rewindLastUserMessage: () => {
    const { sessions, activeSessionId } = get();
    const sessionId = activeSessionId;
    if (!sessionId) return;
    const session = sessions.find((item) => item.id === sessionId);
    if (!session) return;
    let lastUserId: string | null = null;
    for (let i = session.messages.length - 1; i >= 0; i -= 1) {
      const message = session.messages[i];
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

export const selectActiveMessages = (state: SessionsStore): ChatMessage[] =>
  selectActiveSession(state)?.messages ?? EMPTY_MESSAGES;
