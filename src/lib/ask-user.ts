import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { AskUserAnswerEntry, AskUserQuestion, PendingQuestionState } from "@/types/chat";

type QuestionsStore = {
  byCallId: Record<string, PendingQuestionState>;
  upsert: (state: PendingQuestionState) => void;
  setAnswer: (callId: string, answer: AskUserAnswerEntry) => void;
  submit: (callId: string) => Promise<{ error?: string }>;
  cancel: (callId: string) => Promise<{ error?: string }>;
};

export const useAskUserStore = create<QuestionsStore>((set, get) => ({
  byCallId: {},

  upsert: (state) => {
    set((prev) => {
      const current = prev.byCallId[state.callId];
      const messageId = state.messageId ?? current?.messageId ?? null;
      const next = { ...state, messageId };
      return {
        byCallId: { ...prev.byCallId, [state.callId]: next },
      };
    });
  },

  setAnswer: (callId, answer) => {
    set((prev) => {
      const current = prev.byCallId[callId];
      if (!current) return prev;
      const nextAnswers = current.questions.map((question) => {
        const existing = current.answers.find((entry) => entry.questionId === question.id);
        if (question.id === answer.questionId) {
          return answer;
        }
        return (
          existing ?? {
            questionId: question.id,
            selected: [],
            freeText: "",
          }
        );
      });
      return {
        byCallId: {
          ...prev.byCallId,
          [callId]: { ...current, answers: nextAnswers },
        },
      };
    });
  },

  submit: async (callId) => {
    const state = get().byCallId[callId];
    if (!state) return { error: "Question not found." };
    if (state.questions.length === 0) return { error: "Question is empty." };
    const answers: AskUserAnswerEntry[] = state.questions.map((question) => {
      const entry = state.answers.find((item) => item.questionId === question.id);
      if (!entry) {
        return { questionId: question.id, selected: [], freeText: "", skipped: true };
      }
      return entry;
    });
    try {
      await invoke<boolean>("submit_ask_user_answer", { callId, answers });
    } catch (error) {
      return { error: String(error) };
    }
    set((prev) => {
      if (!prev.byCallId[callId]) return prev;
      const { [callId]: _removed, ...rest } = prev.byCallId;
      return { byCallId: rest };
    });
    return {};
  },

  cancel: async (callId) => {
    try {
      await invoke<boolean>("cancel_ask_user_answer", { callId });
    } catch (error) {
      return { error: String(error) };
    }
    set((prev) => {
      const { [callId]: _removed, ...rest } = prev.byCallId;
      return { byCallId: rest };
    });
    return {};
  },
}));

export type { AskUserQuestion };
