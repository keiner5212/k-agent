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

  it("preserves session todos so the UI can restore after reload", () => {
    const session = sanitizeSessionRecord({
      id: "s1",
      todos: [
        { id: "step-1", content: "Investigate ask_user", status: "in_progress", priority: 9 },
        { id: "step-2", content: "Wire todowrite", status: "pending", priority: 5 },
      ],
    });
    expect(session.todos).toEqual([
      { id: "step-1", content: "Investigate ask_user", status: "in_progress", priority: 9 },
      { id: "step-2", content: "Wire todowrite", status: "pending", priority: 5 },
    ]);
  });

  it("migrates legacy todos (no id, string priority) to the new shape", () => {
    const session = sanitizeSessionRecord({
      id: "s1",
      todos: [
        { content: "Legacy task one", status: "in_progress", priority: "high" },
        { content: "Legacy task two", status: "pending", priority: "low" },
        { content: "Legacy task three", status: "completed", priority: "medium" },
      ],
    });
    expect(session.todos ?? []).toHaveLength(3);
    expect(session.todos?.[0]).toMatchObject({
      content: "Legacy task one",
      status: "in_progress",
      priority: 9,
    });
    expect(session.todos?.[1]).toMatchObject({
      content: "Legacy task two",
      status: "pending",
      priority: 1,
    });
    expect(session.todos?.[2]).toMatchObject({
      content: "Legacy task three",
      status: "completed",
      priority: 5,
    });
    expect(session.todos?.[0]?.id).toMatch(/^legacy-/);
    expect(session.todos?.[0]?.id).not.toBe(session.todos?.[1]?.id);
  });

  it("defaults todos and history to empty arrays when missing", () => {
    const session = sanitizeSessionRecord({ id: "s1" });
    expect(session.todos).toEqual([]);
    expect(session.todosHistory).toEqual([]);
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

  it("sanitizes todos_history entries without a diff field", () => {
    const snapshot = sanitizeSessionsSnapshot({
      activeSessionId: "s1",
      sessions: [
        {
          id: "s1",
          todosHistory: [
            { timestamp: 1 },
            {
              timestamp: 2,
              diff: { added: [{ id: "x", content: "x", status: "pending", priority: 5 }] },
            },
            { timestamp: 3, diff: null },
          ],
        },
      ],
    });
    expect(snapshot.sessions[0]?.todosHistory).toEqual([
      { timestamp: 1, diff: {} },
      {
        timestamp: 2,
        diff: { added: [{ id: "x", content: "x", status: "pending", priority: 5 }] },
      },
      { timestamp: 3, diff: {} },
    ]);
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
