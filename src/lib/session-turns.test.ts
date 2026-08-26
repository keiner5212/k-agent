import { describe, expect, it } from "vitest";
import {
  sanitizeSessionRecord,
  sanitizeSessionsSnapshot,
  sessionMessages,
  toChatTurns,
} from "./session-turns";
import type { ChatMessage } from "@/types/chat";

describe("sessionMessages", () => {
  it("returns empty array when messages is missing", () => {
    expect(sessionMessages({ id: "s1" } as { id: string; messages?: ChatMessage[] })).toEqual([]);
    expect(sessionMessages(null).length).toBe(0);
    expect(sessionMessages(undefined).length).toBe(0);
  });
});

describe("sanitizeSessionRecord", () => {
  it("fills missing messages so send can read length", () => {
    const session = sanitizeSessionRecord({ id: "s1" });
    expect(session.messages).toEqual([]);
    expect(session.messages.length).toBe(0);
  });
});

describe("sanitizeSessionsSnapshot", () => {
  it("maps omitted session messages to empty arrays", () => {
    const snapshot = sanitizeSessionsSnapshot({
      activeSessionId: "s1",
      sessions: [{ id: "s1", title: "Chat" }],
    });
    expect(snapshot.sessions[0]?.messages).toEqual([]);
  });
});

describe("toChatTurns", () => {
  it("skips streaming assistant placeholders", () => {
    const messages: ChatMessage[] = [
      { id: "u1", role: "user", content: "hi" },
      { id: "a1", role: "assistant", content: "", streaming: true },
    ];
    const turns = toChatTurns(messages);
    expect(turns).toEqual([
      {
        role: "user",
        content: "hi",
        reasoning: null,
        reasoningSignature: null,
        attachments: undefined,
      },
    ]);
  });

  it("expands tool rounds into assistant calls then user results", () => {
    const messages: ChatMessage[] = [
      { id: "u1", role: "user", content: "read it" },
      {
        id: "a1",
        role: "assistant",
        content: "done",
        toolRounds: [
          {
            reasoning: "think",
            calls: [
              {
                id: "call_1",
                name: "read",
                output: "file body",
              },
            ],
          },
        ],
      },
    ];
    const turns = toChatTurns(messages);
    expect(turns.map((turn) => turn.role)).toEqual(["user", "assistant", "user", "assistant"]);
    expect(turns[1]?.toolCalls?.[0]?.name).toBe("read");
    expect(turns[2]?.toolResult).toEqual({
      callId: "call_1",
      name: "read",
      content: "file body",
    });
    expect(turns[3]?.content).toBe("done");
  });
});
