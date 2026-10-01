import { describe, expect, it } from "vitest";
import { activateWorkspace, adoptUnscopedSessions, sessionInWorkspace } from "@/types/sessions";
import {
  applySystemReminder,
  messagesBeforeTail,
  sanitizeSessionRecord,
  sanitizeSessionsSnapshot,
  sessionMessages,
  summaryTranscript,
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

describe("workspace sessions", () => {
  const bare = {
    id: "s1",
    title: "",
    preview: "",
    updatedAt: 1,
    messages: [],
  };

  it("leaves a missing workspace empty and keeps a stored path", () => {
    expect(sanitizeSessionRecord({ id: "s1" }).workspacePath).toBeUndefined();
    expect(sanitizeSessionRecord({ id: "s1", workspacePath: "/work/a" }).workspacePath).toBe(
      "/work/a",
    );
  });

  it("attaches old chats to the open workspace once", () => {
    const first = adoptUnscopedSessions([bare], "/work/a/");
    expect(first.changed).toBe(true);
    expect(first.sessions[0]?.workspacePath).toBe("/work/a");
    const second = adoptUnscopedSessions(first.sessions, "/work/b");
    expect(second.changed).toBe(false);
    expect(second.sessions[0]?.workspacePath).toBe("/work/a");
  });

  it("shows only the open workspace and opens a blank chat when it has none", () => {
    const sessions = [
      { ...bare, id: "a", workspacePath: "/work/a", updatedAt: 2 },
      { ...bare, id: "b", workspacePath: "/work/b", updatedAt: 9 },
    ];
    expect(sessions.filter((session) => sessionInWorkspace(session, "/work/a/"))).toEqual([
      sessions[0],
    ]);
    const focused = activateWorkspace(sessions, "b", "/work/a", () => "new");
    expect(focused.activeSessionId).toBe("a");
    expect(focused.sessions).toHaveLength(2);
    const empty = activateWorkspace(sessions, "b", "/work/c", () => "new");
    expect(empty.activeSessionId).toBe("new");
    expect(empty.sessions.find((session) => session.id === "new")?.workspacePath).toBe("/work/c");
  });
});

describe("context memory", () => {
  const user = (id: string, content: string): ChatMessage => ({ id, role: "user", content });

  it("keeps the last two messages out of the summary", () => {
    const messages = [user("1", "goal"), user("2", "middle"), user("3", "latest")];
    const split = messagesBeforeTail(messages);
    expect(split.head.map((message) => message.id)).toEqual(["1"]);
    expect(split.tail.map((message) => message.id)).toEqual(["2", "3"]);
    expect(messagesBeforeTail([user("1", "only"), user("2", "two")]).head).toEqual([]);
  });

  it("keeps the first line and the recent tail inside the transcript budget", () => {
    const messages = [user("1", "goal"), user("2", "x".repeat(90_000)), user("3", "recent")];
    const text = summaryTranscript(messages);
    expect(text.startsWith("user: goal")).toBe(true);
    expect(text.includes("recent")).toBe(true);
    expect(text.includes("x".repeat(4000))).toBe(true);
    expect(text.includes("x".repeat(4001))).toBe(false);
  });

  it("repeats the system text on the reminder interval", () => {
    const turns = Array.from({ length: 8 }, (_, index) => ({
      role: "user" as const,
      content: `m${index}`,
    }));
    const reminded = applySystemReminder(turns, "Reply in English.", 8);
    expect(reminded[7]?.content).toContain("<system-reminder>");
    expect(reminded[7]?.content).toContain("Reply in English.");
    expect(reminded[0]?.content).toBe("m0");
    const early = applySystemReminder(turns.slice(0, 7), "Reply in English.", 8);
    expect(early[6]?.content).toBe("m6");
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
