import type { ChatMessage, ChatTurn } from "@/types/chat";
import type { SessionRecord, SessionsSnapshot } from "@/types/sessions";

type LooseSession = {
  id: string;
  title?: string;
  preview?: string;
  updatedAt?: number;
  messages?: ChatMessage[] | null;
};

export const sessionMessages = (
  session: { messages?: ChatMessage[] | null } | null | undefined,
): ChatMessage[] => (Array.isArray(session?.messages) ? session.messages : []);

export const sanitizeSessionRecord = (session: LooseSession): SessionRecord => ({
  id: session.id,
  title: session.title ?? "",
  preview: session.preview ?? "",
  updatedAt: typeof session.updatedAt === "number" ? session.updatedAt : 0,
  messages: sessionMessages(session),
});

export const sanitizeSessionsSnapshot = (snapshot: {
  activeSessionId: string;
  sessions?: LooseSession[] | null;
}): SessionsSnapshot => ({
  activeSessionId: snapshot.activeSessionId,
  sessions: (snapshot.sessions ?? []).map(sanitizeSessionRecord),
});

export const toChatTurns = (messages: ChatMessage[]): ChatTurn[] => {
  const turns: ChatTurn[] = [];
  for (const message of messages) {
    if (message.streaming) continue;
    if (message.role === "assistant") {
      const rounds = message.toolRounds ?? [];
      const splitReasoning = rounds.length > 0;
      for (const round of rounds) {
        const calls = round.calls ?? [];
        turns.push({
          role: "assistant",
          content: round.content ?? "",
          reasoning: round.reasoning || null,
          reasoningSignature: round.reasoningSignature ?? null,
          attachments: message.attachments,
          toolCalls: calls.map((call) => ({
            id: call.id,
            name: call.name,
            argument: call.argument,
            arguments: call.arguments,
            thoughtSignature: call.thoughtSignature,
          })),
        });
        for (const call of calls) {
          turns.push({
            role: "user",
            content: "",
            toolResult: {
              callId: call.id,
              name: call.name,
              content: call.output,
            },
          });
        }
      }
      const body = message.shellAiSummary ?? message.content;
      if (
        body.trim().length > 0 ||
        (message.attachments?.length ?? 0) > 0 ||
        (!splitReasoning && rounds.length === 0 && (message.reasoning ?? "").trim().length > 0)
      ) {
        turns.push({
          role: "assistant",
          content: body,
          reasoning: splitReasoning || rounds.length === 0 ? (message.reasoning ?? null) : null,
          reasoningSignature:
            splitReasoning || rounds.length === 0 ? (message.reasoningSignature ?? null) : null,
          attachments: message.attachments,
        });
      }
      continue;
    }
    if (
      message.content.trim().length > 0 ||
      (message.reasoning ?? "").trim().length > 0 ||
      (message.attachments?.length ?? 0) > 0
    ) {
      turns.push({
        role: message.role,
        content: message.shellAiSummary ?? message.content,
        reasoning: message.reasoning ?? null,
        reasoningSignature: message.reasoningSignature ?? null,
        attachments: message.attachments,
      });
    }
  }
  return turns;
};
