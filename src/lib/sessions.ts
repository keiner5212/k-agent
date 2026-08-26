import { create } from "zustand";
import { Channel, invoke } from "@tauri-apps/api/core";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { useProvidersStore } from "@/lib/providers";
import { selectEffort, useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
import type { ChatMessage, SelectedModel, SendChatResult } from "@/types/chat";
import {
  sortSessions,
  titleFromFirstMessage,
  type SessionRecord,
  type SessionsSnapshot,
} from "@/types/sessions";

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
  hydrated: boolean;
  sending: boolean;
  error?: string;
  hydrate: () => Promise<void>;
  create: () => void;
  select: (id: string) => void;
  remove: (id: string) => void;
  send: (text: string) => Promise<void>;
};

export const useSessionsStore = create<SessionsStore>((set, get) => ({
  sessions: [],
  activeSessionId: null,
  sendingSessionId: null,
  hydrated: false,
  sending: false,

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
    const { sessions, activeSessionId, sendingSessionId } = get();
    const nextSessions = sessions.filter((session) => session.id !== id);
    const stopSending = sendingSessionId === id;
    if (nextSessions.length === 0) {
      const session = emptySession();
      const seeded = [session];
      set({
        sessions: seeded,
        activeSessionId: session.id,
        error: undefined,
        ...(stopSending ? { sending: false, sendingSessionId: null } : {}),
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
    });
    void persistSnapshot(snapshotFromState(sorted, nextActiveId));
  },

  send: async (text) => {
    const trimmed = text.trim();
    if (!trimmed || get().sending) return;
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
    const userMessage: ChatMessage = {
      id: nextId(),
      role: "user",
      content: trimmed,
    };
    const now = Date.now();

    const withUser = sortSessions(
      patchActiveSession(ensured.sessions, sessionId, (session) => ({
        ...session,
        preview: trimmed,
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
      const onChunk = new Channel<string>();
      onChunk.onmessage = (chunk) => {
        if (!chunk || get().sendingSessionId !== sessionId) return;
        const nextSessions = get().sessions.map((session) => {
          if (session.id !== sessionId) return session;
          const index = session.messages.findIndex((message) => message.id === assistantId);
          if (index < 0) {
            return {
              ...session,
              preview: chunk,
              messages: [
                ...session.messages,
                { id: assistantId, role: "assistant" as const, content: chunk, streaming: true },
              ],
            };
          }
          const messages = session.messages.slice();
          const current = messages[index];
          if (!current) return session;
          const content = `${current.content}${chunk}`;
          messages[index] = { ...current, content, streaming: true };
          return { ...session, preview: content, messages };
        });
        set({ sessions: nextSessions });
      };

      const result = await invoke<SendChatResult>("send_chat_message", {
        input: {
          providerId: selection.providerId,
          modelId: selection.modelId,
          message: trimmed,
          effort: effort ?? null,
        },
        onChunk,
      });
      const replyAt = Date.now();
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
              { id: assistantId, role: "assistant" as const, content: result.content },
            ],
          };
        }
        const messages = session.messages.slice();
        const current = messages[index];
        if (!current) return session;
        messages[index] = {
          ...current,
          content: result.content || current.content,
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
}));

export const selectActiveSession = (state: SessionsStore): SessionRecord | null => {
  const { sessions, activeSessionId } = state;
  if (!activeSessionId) return null;
  return sessions.find((session) => session.id === activeSessionId) ?? null;
};

const EMPTY_MESSAGES: ChatMessage[] = [];

export const selectActiveMessages = (state: SessionsStore): ChatMessage[] =>
  selectActiveSession(state)?.messages ?? EMPTY_MESSAGES;
