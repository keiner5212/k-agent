import { slashCommandByName, withPromptCommands } from "@/lib/slash-commands";
import { useSettingsStore } from "@/lib/settings";
import { findSkillByName, flattenSkills } from "@/lib/skill-mentions";
import { useSkillsStore } from "@/lib/skills";
import type { ChatTurn } from "@/types/chat";

const slashTokenRe = (): RegExp => /\/([a-zA-Z][\w-]*)\s+\([^)]+\)/g;

const trailingSameLineArgs = (text: string, from: number): { args: string; consumed: number } => {
  const tail = text.slice(from);
  const newline = tail.indexOf("\n");
  const sameLine = newline === -1 ? tail : tail.slice(0, newline);
  const consumed = newline === -1 ? tail.length : newline;
  return { args: sameLine.trim(), consumed };
};

export const expandPromptCommands = (text: string): string => {
  let expanded = text;
  const tokenRe = slashTokenRe();
  let searchFrom = 0;
  while (true) {
    tokenRe.lastIndex = searchFrom;
    const match = tokenRe.exec(expanded);
    if (!match) break;
    const index = match.index;
    const name = match[1] ?? "";
    const tokenLength = match[0]?.length ?? 0;
    const command = slashCommandByName(
      name,
      withPromptCommands(useSettingsStore.getState().promptCommands),
    );
    if (!command || command.kind !== "template") {
      searchFrom = index + tokenLength;
      continue;
    }
    const afterToken = index + tokenLength;
    const { args, consumed } = trailingSameLineArgs(expanded, afterToken);
    const replacement = command.template.replace("$ARGUMENTS", args);
    const end = afterToken + consumed;
    expanded = `${expanded.slice(0, index)}${replacement}${expanded.slice(end)}`;
    searchFrom = index + replacement.length;
  }
  return expanded;
};

export const expandPromptCommandsInTurns = (turns: readonly ChatTurn[]): ChatTurn[] =>
  turns.map((turn) => {
    if (turn.role !== "user" || turn.toolResult || !turn.content.includes("/")) return turn;
    const content = expandPromptCommands(turn.content);
    return content === turn.content ? turn : { ...turn, content };
  });

export const expandComposerText = async (
  text: string,
): Promise<{ text: string; error?: string }> => {
  let expanded = expandPromptCommands(text);
  const skills = flattenSkills(useSkillsStore.getState().contexts);
  const readFile = useSkillsStore.getState().readFile;
  const skillContents = new Map<string, string>();
  const tokenRe = slashTokenRe();

  let searchFrom = 0;
  while (true) {
    tokenRe.lastIndex = searchFrom;
    const match = tokenRe.exec(expanded);
    if (!match) break;
    const index = match.index;
    const name = match[1] ?? "";
    const tokenLength = match[0]?.length ?? 0;
    const command = slashCommandByName(
      name,
      withPromptCommands(useSettingsStore.getState().promptCommands),
    );
    if (command) {
      searchFrom = index + tokenLength;
      continue;
    }
    const skill = findSkillByName(skills, name);
    if (!skill) {
      searchFrom = index + tokenLength;
      continue;
    }
    if (!skillContents.has(skill.name)) {
      const result = await readFile(skill.path);
      if (result.error) return { text, error: result.error };
      skillContents.set(skill.name, result.content ?? "");
    }
    const content = skillContents.get(skill.name) ?? "";
    expanded = `${expanded.slice(0, index)}${content}${expanded.slice(index + tokenLength)}`;
    searchFrom = index + content.length;
  }

  return { text: expanded };
};
