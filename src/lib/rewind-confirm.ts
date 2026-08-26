import { create } from "zustand";
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
  const session = useSessionsStore
    .getState()
    .sessions.find((item) => item.id === sessionId);
  if (!session) return null;
  for (let i = session.messages.length - 1; i >= 0; i -= 1) {
    const message = session.messages[i];
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
