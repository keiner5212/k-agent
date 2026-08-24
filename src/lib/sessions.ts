import { create } from "zustand";
import type { SessionSummary } from "@/types/sessions";

type SessionsStore = {
  sessions: SessionSummary[];
  remove: (id: string) => void;
};

export const useSessionsStore = create<SessionsStore>((set) => ({
  sessions: [],
  remove: (id) => {
    set((state) => ({ sessions: state.sessions.filter((session) => session.id !== id) }));
  },
}));
