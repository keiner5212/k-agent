import { expandWorkspaceMentions } from "@/lib/file-mentions";
import { shellQuote } from "@/lib/shell";
import { useWorkspaceFilesStore } from "@/lib/workspace-files";

export type OutgoingMentionFormat = "plain" | "shell";

const mentionQueriesInText = (text: string): string[] => {
  const queries: string[] = [];
  for (const match of text.matchAll(/@([^\s@]+)/g)) {
    const query = (match[1] ?? "").replace(/\/+$/, "");
    if (query.length > 0) queries.push(query);
  }
  return queries;
};

export const resolveOutgoingMentions = async (
  text: string,
  workspaceRoot: string,
  format: OutgoingMentionFormat,
): Promise<string> => {
  if (!text.includes("@")) return text;
  const store = useWorkspaceFilesStore.getState();
  for (const query of mentionQueriesInText(text)) {
    await store.ensureMentionScope(query);
  }
  const entries = Object.values(store.dirs).flatMap((dir) => dir.entries);
  const render = format === "shell" ? shellQuote : (absolute: string): string => absolute;
  return expandWorkspaceMentions(text, entries, workspaceRoot, render);
};
