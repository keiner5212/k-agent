import { slashCommandByName } from "@/lib/slash-commands";
import { findSkillByName, flattenSkills } from "@/lib/skill-mentions";
import { useSkillsStore } from "@/lib/skills";

const trailingSameLineArgs = (text: string, from: number): { args: string; consumed: number } => {
  const tail = text.slice(from);
  const newline = tail.indexOf("\n");
  const sameLine = newline === -1 ? tail : tail.slice(0, newline);
  const consumed = newline === -1 ? tail.length : newline;
  return { args: sameLine.trim(), consumed };
};

export const expandComposerText = async (
  text: string,
): Promise<{ text: string; error?: string }> => {
  let expanded = text;
  const skills = flattenSkills(useSkillsStore.getState().contexts);
  const readFile = useSkillsStore.getState().readFile;
  const skillContents = new Map<string, string>();
  const slashTokenRe = /\/([a-zA-Z][\w-]*)\s+\([^)]+\)/g;

  let searchFrom = 0;
  while (true) {
    slashTokenRe.lastIndex = searchFrom;
    const match = slashTokenRe.exec(expanded);
    if (!match) break;
    const index = match.index;
    const name = match[1] ?? "";
    const tokenLength = match[0]?.length ?? 0;
    const command = slashCommandByName(name);
    if (command) {
      const afterToken = index + tokenLength;
      const { args, consumed } = trailingSameLineArgs(expanded, afterToken);
      const replacement = command.template.replace("$ARGUMENTS", args);
      const end = afterToken + consumed;
      expanded = `${expanded.slice(0, index)}${replacement}${expanded.slice(end)}`;
      searchFrom = index + replacement.length;
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
