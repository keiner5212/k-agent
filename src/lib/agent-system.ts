import type { TFunction } from "i18next";
import { resolveAgentMeta } from "@/lib/builtin-agents";
import type { AgentContext, AgentMeta, AgentSkillRef } from "@/types/agents";
import type { AgentsMdFile } from "@/types/agents-md";
import type { SkillContext } from "@/types/skills";

const findSkillMeta = (
  contexts: readonly SkillContext[],
  ref: AgentSkillRef,
): { name: string; description: string } | null => {
  const ctx = contexts.find((item) => item.kind === ref.kind);
  const skill = ctx?.skills.find((item) => item.id === ref.id);
  if (!skill) return null;
  const name = skill.name.trim().length > 0 ? skill.name.trim() : ref.id;
  const description = skill.description.trim();
  return { name, description };
};

const uniqueAgentSkills = (
  refs: readonly AgentSkillRef[],
  skillContexts: readonly SkillContext[],
): { name: string; description: string }[] => {
  const out: { name: string; description: string }[] = [];
  const seen = new Set<string>();
  for (const ref of refs) {
    const meta = findSkillMeta(skillContexts, ref);
    if (!meta || seen.has(meta.name)) continue;
    seen.add(meta.name);
    out.push(meta);
  }
  return out;
};

const formatSkillLines = (items: { name: string; description: string }[]): string[] =>
  items.map((item) => {
    const detail = item.description.length > 0 ? item.description : item.name;
    return `- ${item.name}: ${detail}`;
  });

const buildAgentSkillCatalog = (items: { name: string; description: string }[]): string => {
  if (items.length === 0) return "";
  return ["# Agent skills", ...formatSkillLines(items)].join("\n");
};

const buildWorkspaceSkillCatalog = (skillContexts: readonly SkillContext[]): string => {
  const ctx = skillContexts.find((item) => item.kind === "local");
  const items: { name: string; description: string }[] = [];
  for (const skill of ctx?.skills ?? []) {
    const name = skill.name.trim().length > 0 ? skill.name.trim() : skill.id;
    items.push({ name, description: skill.description.trim() });
  }
  if (items.length === 0) return "";
  return ["# Workspace skills", ...formatSkillLines(items)].join("\n");
};

const buildTurnProtocol = (
  agentSkillNames: readonly string[],
  hasWorkspaceSkills: boolean,
): string => {
  const lines = ["# Flow"];
  if (agentSkillNames.length > 0) {
    lines.push(`Turn 1: one skill batch for ${agentSkillNames.join(", ")}. No prose.`);
  }
  if (hasWorkspaceSkills) {
    lines.push("Later: skill for a listed workspace skill when relevant.");
  }
  lines.push("Use only advertised tools. Then follow Personality.");
  return lines.join("\n");
};

export const buildAgentsMdRules = (files: readonly AgentsMdFile[]): string => {
  const parts: string[] = [];
  const globalBody =
    files.find((item) => item.kind === "global" && item.exists)?.content.trim() ?? "";
  if (globalBody.length > 0) parts.push(`# Global rules\n\n${globalBody}`);
  const workspaceBody =
    files.find((item) => item.kind === "local" && item.exists)?.content.trim() ?? "";
  if (workspaceBody.length > 0) parts.push(`# Workspace rules\n\n${workspaceBody}`);
  return parts.join("\n\n");
};

export const composeAgentSystem = (
  agent: AgentMeta | null,
  skillContexts: SkillContext[],
  loadedSkillNames: readonly string[] = [],
): string => {
  if (!agent) return "";
  const loaded = new Set(loadedSkillNames.map((name) => name.trim()).filter(Boolean));
  const agentSkills = uniqueAgentSkills(agent.skills, skillContexts).filter(
    (item) => !loaded.has(item.name),
  );
  const workspaceSkills = buildWorkspaceSkillCatalog(skillContexts);
  const parts: string[] = [
    buildTurnProtocol(
      agentSkills.map((item) => item.name),
      workspaceSkills.length > 0,
    ),
  ];
  const agentCatalog = buildAgentSkillCatalog(agentSkills);
  if (agentCatalog.length > 0) parts.push(agentCatalog);
  if (workspaceSkills.length > 0) parts.push(workspaceSkills);
  const personality = agent.personality.trim();
  if (personality.length > 0) {
    parts.push(`# Personality\n\n${personality}`);
  }
  return parts.join("\n\n");
};

export const resolveAgentSystem = (
  key: string,
  agentContexts: AgentContext[],
  skillContexts: SkillContext[],
  t: TFunction,
): string => composeAgentSystem(resolveAgentMeta(key, agentContexts, t), skillContexts);
