import { estimateTokensFromText } from "@/lib/jobs-handlers";

export type ActiveSlashCommand = {
  start: number;
  end: number;
  query: string;
};

export type SlashCommandKind = "template" | "action";

export type SlashCommandDef = {
  id: string;
  name: string;
  kind: SlashCommandKind;
  descriptionKey: string;
  template: string;
  estimatedTokens: number;
};

const REVIEW_TEMPLATE = `You are a code reviewer. Review code changes and return an actionable report.

Input: $ARGUMENTS

## What to review (default)
When Input is empty:
1. Run git diff --cached for staged changes.
2. Run git diff for unstaged changes.
3. Run git status --short for untracked files.
If there are no uncommitted changes, review the latest commit with git show HEAD.

After finding changed files, read full file contents for context beyond the diff.

## Focus
- Bugs, security issues, correctness, missing error handling
- Whether changes match existing patterns in the codebase
- Only flag real issues; investigate before claiming a bug

## Output
Structured report. Direct tone. No praise. State severity clearly.`;

const buildSlashCommand = (
  id: string,
  name: string,
  descriptionKey: string,
  template: string,
): SlashCommandDef => ({
  id,
  name,
  kind: "template",
  descriptionKey,
  template,
  estimatedTokens: estimateTokensFromText(template),
});

const buildActionCommand = (id: string, name: string, descriptionKey: string): SlashCommandDef => ({
  id,
  name,
  kind: "action",
  descriptionKey,
  template: "",
  estimatedTokens: 0,
});

export const SLASH_COMMANDS: SlashCommandDef[] = [
  buildSlashCommand("review", "review", "chat.slashCommands.review.description", REVIEW_TEMPLATE),
  buildActionCommand("undo", "undo", "chat.slashCommands.undo.description"),
];

export const SLASH_COMMAND_RESULT_LIMIT = 20;

export const SLASH_COMMAND_NAMES: ReadonlySet<string> = new Set(
  SLASH_COMMANDS.flatMap((command) => [command.name, command.name.toLowerCase()]),
);

export const parseActiveSlashCommand = (
  text: string,
  cursor: number,
): ActiveSlashCommand | null => {
  const before = text.slice(0, cursor);
  const slashIndex = before.lastIndexOf("/");
  if (slashIndex === -1) return null;
  if (slashIndex > 0 && !/\s/.test(before.charAt(slashIndex - 1))) return null;
  const query = before.slice(slashIndex + 1);
  if (query.length > 0 && /\s/.test(query)) return null;
  return { start: slashIndex, end: cursor, query };
};

export const filterSlashCommands = (query: string): SlashCommandDef[] => {
  const lower = query.toLowerCase();
  const items: SlashCommandDef[] = [];
  for (const command of SLASH_COMMANDS) {
    if (lower.length > 0 && !command.name.toLowerCase().startsWith(lower)) continue;
    items.push(command);
    if (items.length >= SLASH_COMMAND_RESULT_LIMIT) break;
  }
  return items;
};

export const slashCommandByName = (name: string): SlashCommandDef | undefined =>
  SLASH_COMMANDS.find((command) => command.name === name);

const ACTION_TOKEN_RE = /\s*\((?:~\d+(?:\.\d+)?(?:[KMB])?)\)\s*$/;

export const matchActionSlashCommand = (text: string): SlashCommandDef | null => {
  const trimmed = text.trim();
  if (trimmed.length === 0) return null;
  const match = trimmed.match(/^\/([a-zA-Z][\w-]*)/);
  if (!match) return null;
  const command = slashCommandByName(match[1] ?? "");
  if (!command || command.kind !== "action") return null;
  const rest = trimmed.slice(match[0].length).replace(ACTION_TOKEN_RE, "").trim();
  if (rest.length > 0) return null;
  return command;
};
