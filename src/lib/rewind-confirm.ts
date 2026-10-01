import { create } from "zustand";
import { sessionMessages } from "@/lib/session-turns";
import { useSessionsStore } from "@/lib/sessions";

type RewindConfirmState = {
  open: boolean;
  targetMessageId: string | null;
  requestRewind: (messageId?: string) => void;
  cancel: () => void;
};

const lastUserMessageId = (): string | null => {
  const sessionId = useSessionsStore.getState().activeSessionId;
  if (!sessionId) return null;
  const session = useSessionsStore.getState().sessions.find((item) => item.id === sessionId);
  if (!session) return null;
  const messages = sessionMessages(session);
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message?.role === "user") return message.id;
  }
  return null;
};

export const useRewindConfirmStore = create<RewindConfirmState>((set) => ({
  open: false,
  targetMessageId: null,
  requestRewind: (messageId) => {
    const resolved = messageId ?? lastUserMessageId();
    if (!resolved) return;
    set({ open: true, targetMessageId: resolved });
  },
  cancel: () => set({ open: false, targetMessageId: null }),
}));
