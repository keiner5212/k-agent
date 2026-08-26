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

const buildAgentSkillCatalog = (items: { name: string; description: string }[]): string => {
  const lines: string[] = [];
  for (const meta of items) {
    const detail = meta.description.length > 0 ? meta.description : meta.name;
    lines.push(`- \`${meta.name}\`: ${detail}`);
  }
  if (lines.length === 0) return "";
  return ["# 3. Agent skills", "", ...lines].join("\n");
};

const buildWorkspaceSkillCatalog = (skillContexts: readonly SkillContext[]): string => {
  const ctx = skillContexts.find((item) => item.kind === "local");
  const lines: string[] = [];
  for (const skill of ctx?.skills ?? []) {
    const name = skill.name.trim().length > 0 ? skill.name.trim() : skill.id;
    const description = skill.description.trim();
    const detail = description.length > 0 ? description : name;
    lines.push(`- \`${name}\`: ${detail}`);
  }
  if (lines.length === 0) return "";
  return ["# 4. Workspace skills", "", ...lines].join("\n");
};

const buildAgentSkillLoadingDirective = (names: readonly string[]): string => {
  if (names.length === 0) return "";
  const count = names.length;
  const plural = count === 1 ? "" : "s";
  const list = names.map((name, index) => `${index + 1}. \`${name}\``).join("\n");
  return [
    "# 2. Agent skill loading - turn 1",
    "",
    `Single tool batch, exactly ${count} \`skill\` call${plural}, before any other tool or prose:`,
    "",
    list,
    "",
    "No other tool calls on turn 1. Retry a failed skill once; report on turn 2.",
    "Later turns: load any matching skill not already in context.",
  ].join("\n");
};

const buildTurnProtocol = (hasAgentSkills: boolean): string => {
  const steps = [
    "Follow system sections in order. Each numbered block maps to a phase of the reply.",
    hasAgentSkills
      ? "Turn 1: batch-load every agent-bound skill with the `skill` tool. No prose."
      : "Turn 1: skip agent skill loading when the agent has no bound skills.",
    "Then load matching workspace skills with `skill` when they are listed and relevant.",
    "Call only tools in the request tools list (local and MCP). Do not invent names.",
    "Finally answer using the agent personality at the end of this system prompt.",
  ];
  return ["# 1. Agent flow", "", ...steps].join("\n");
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
  const parts: string[] = [];
  const protocol = buildTurnProtocol(agent.skills.length > 0);
  if (protocol.length > 0) parts.push(protocol);
  const skillDirective = buildAgentSkillLoadingDirective(agentSkills.map((item) => item.name));
  if (skillDirective.length > 0) parts.push(skillDirective);
  const agentCatalog = buildAgentSkillCatalog(agentSkills);
  if (agentCatalog.length > 0) parts.push(agentCatalog);
  const workspaceSkills = buildWorkspaceSkillCatalog(skillContexts);
  if (workspaceSkills.length > 0) parts.push(workspaceSkills);
  const personality = agent.personality.trim();
  if (personality.length > 0) {
    parts.push(`# 5. Personality\n\n${personality}`);
  }
  return parts.join("\n\n");
};

export const resolveAgentSystem = (
  key: string,
  agentContexts: AgentContext[],
  skillContexts: SkillContext[],
  t: TFunction,
): string => composeAgentSystem(resolveAgentMeta(key, agentContexts, t), skillContexts);
