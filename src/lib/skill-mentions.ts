import { formatContextWindow } from "@/types/providers";
import { MENTION_RESULT_LIMIT } from "@/lib/file-mentions";
import type { SkillContext, SkillInfo } from "@/types/skills";

export type SkillFilterResult = {
  items: SkillInfo[];
  tooMany: boolean;
};

type SlashSpan = {
  start: number;
  end: number;
};

export const flattenSkills = (contexts: readonly SkillContext[]): SkillInfo[] => {
  const local = contexts.find((context) => context.kind === "local");
  const global = contexts.find((context) => context.kind === "global");
  return [...(local?.skills ?? []), ...(global?.skills ?? [])];
};

export const formatInlineTokenCount = (tokens: number): string =>
  `(~${formatContextWindow(tokens)})`;

export const filterSkills = (skills: readonly SkillInfo[], prefix: string): SkillFilterResult => {
  const lowerPrefix = prefix.toLowerCase();
  const items: SkillInfo[] = [];
  for (const skill of skills) {
    if (lowerPrefix.length > 0 && !skill.name.toLowerCase().startsWith(lowerPrefix)) continue;
    items.push(skill);
    if (items.length > MENTION_RESULT_LIMIT) {
      return { items: [], tooMany: true };
    }
  }
  return { items, tooMany: false };
};

export const buildSkillNameSet = (skills: readonly SkillInfo[]): Set<string> => {
  const out = new Set<string>();
  for (const skill of skills) {
    out.add(skill.name);
    out.add(skill.name.toLowerCase());
  }
  return out;
};

export const findSkillByName = (
  skills: readonly SkillInfo[],
  name: string,
): SkillInfo | undefined => {
  const lower = name.toLowerCase();
  return skills.find((skill) => skill.name.toLowerCase() === lower);
};

export const applySlashToken = (
  text: string,
  span: SlashSpan,
  name: string,
  tokens: number,
): { next: string; cursor: number } => {
  const before = text.slice(0, span.start);
  const after = text.slice(span.end);
  const token = `/${name} ${formatInlineTokenCount(tokens)} `;
  const next = `${before}${token}${after}`;
  return { next, cursor: before.length + token.length };
};
