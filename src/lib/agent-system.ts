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

const tagged = (name: string, body: string): string => {
  const trimmed = body.trim();
  if (trimmed.length === 0) return "";
  return `<${name}>\n${trimmed}\n</${name}>`;
};

const skillLine = (name: string, description: string): string => {
  const detail = description.length > 0 ? description : name;
  return `- \`${name}\`: ${detail}`;
};

const buildAgentSkills = (items: { name: string; description: string }[]): string => {
  if (items.length === 0) return "";
  const count = items.length;
  const plural = count === 1 ? "" : "s";
  const lines = items.map((meta) => skillLine(meta.name, meta.description));
  return tagged(
    "agent-skills",
    [
      `Turn 1: one tool batch, exactly ${count} \`skill\` call${plural}, before any other tool or prose.`,
      "",
      ...lines,
      "",
      "No other tool calls on turn 1. Retry a failed skill once, then report on the next turn.",
      "Later turns: load a listed skill only when it is not already in context.",
    ].join("\n"),
  );
};

const buildWorkspaceSkills = (
  skillContexts: readonly SkillContext[],
  loaded: ReadonlySet<string>,
): string => {
  const ctx = skillContexts.find((item) => item.kind === "local");
  const lines: string[] = [];
  for (const skill of ctx?.skills ?? []) {
    const name = skill.name.trim().length > 0 ? skill.name.trim() : skill.id;
    if (loaded.has(name)) continue;
    lines.push(skillLine(name, skill.description.trim()));
  }
  if (lines.length === 0) return "";
  return tagged(
    "workspace-skills",
    [
      "Workspace skills in this project. Load a matching one with `skill` when the task needs it.",
      "",
      ...lines,
    ].join("\n"),
  );
};

const buildFlow = (hasAgentSkills: boolean): string =>
  tagged(
    "agent-flow",
    [
      "Follow the tagged sections in this system prompt in order.",
      hasAgentSkills
        ? "Turn 1 loads every skill in <agent-skills> and writes no prose."
        : "Turn 1 has no agent-skill batch when <agent-skills> is absent.",
      "Then load a <workspace-skills> entry only when it matches the task.",
      "Call only tools in the request tools list. Do not invent names.",
      "Answer using <personality>.",
    ].join("\n"),
  );

const CLARIFY = tagged(
  "clarify",
  [
    "Work out exactly what the user is asking before you act.",
    "If any part is not fully clear, call `ask_user` and wait for the answer. Do not guess.",
  ].join("\n"),
);

const TODOS = tagged(
  "todos",
  [
    "Keep the session todo list matched to the work.",
    "When you start, finish, drop, or change a step, call `todowrite` in that same turn.",
    "Do not leave a finished step as pending or in_progress.",
  ].join("\n"),
);

const VISUAL = tagged(
  "visual-check",
  [
    "Use `page_shot` only on a page that is already being served. One shot per review.",
    "Do not repeat it with a different host, height, or selector.",
    "A blank or identical image is a capture miss. Do not edit the page to remove a black box from a bad shot.",
    "Start a dev server with `background`. It is killed when the turn ends. Do not use `bash` for that.",
  ].join("\n"),
);

const MERMAID = tagged(
  "mermaid",
  [
    "Before a `mermaid` fence goes in the reply, call `validate_mermaid` with that exact source.",
    "Include the fence only when the tool returns status ok.",
    "If it returns error, fix the source and validate again. Do not show a diagram that failed the check.",
  ].join("\n"),
);

const hasTool = (agent: AgentMeta, name: string): boolean => agent.tools.includes(name);

const buildToolChoice = (agent: AgentMeta): string => {
  const lines = ["Use the dedicated tool. Do not use `bash` for work another tool already does."];
  if (hasTool(agent, "list_directory")) {
    lines.push("List a directory with `list_directory`. Do not use `ls`, `find`, or `tree`.");
  }
  if (hasTool(agent, "read")) {
    lines.push("Read a file with `read`. Do not use `cat`, `head`, `tail`, or `wc`.");
  }
  if (hasTool(agent, "grep")) {
    lines.push("Search file contents with `grep`. Do not run `grep` or `rg` in the shell.");
  }
  if (hasTool(agent, "write")) lines.push("Create or overwrite a file with `write`.");
  if (hasTool(agent, "edit")) lines.push("Change file contents with `edit`.");
  if (hasTool(agent, "create_folder")) lines.push("Make a directory with `create_folder`.");
  if (hasTool(agent, "delete")) lines.push("Remove a file or empty directory with `delete`.");
  lines.push(
    "`bash` is for a command that must run and finish, such as install, build, test, or git.",
  );
  if (hasTool(agent, "background")) {
    lines.push("A process that must stay up uses `background`, not `bash`.");
  }
  return tagged("tools", lines.join("\n"));
};

const RENDERING = tagged(
  "rendering",
  [
    "Chat output is GitHub-flavored markdown.",
    "- Fenced code blocks with a language hint are syntax-highlighted.",
    "- Fenced `mermaid` blocks render as diagrams after the message finishes streaming. Use them for flows, sequences, and structure that a picture explains faster than prose.",
    "- Do not repeat the same idea in prose when the diagram already shows it.",
  ].join("\n"),
);

export const buildAgentsMdRules = (files: readonly AgentsMdFile[]): string => {
  const parts: string[] = [];
  const globalBody =
    files.find((item) => item.kind === "global" && item.exists)?.content.trim() ?? "";
  if (globalBody.length > 0) parts.push(tagged("global-rules", globalBody));
  const workspaceBody =
    files.find((item) => item.kind === "local" && item.exists)?.content.trim() ?? "";
  if (workspaceBody.length > 0) parts.push(tagged("workspace-rules", workspaceBody));
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
  const flow = buildFlow(agentSkills.length > 0);
  if (flow.length > 0) parts.push(flow);
  const agentSkillBlock = buildAgentSkills(agentSkills);
  if (agentSkillBlock.length > 0) parts.push(agentSkillBlock);
  const workspaceSkills = buildWorkspaceSkills(skillContexts, loaded);
  if (workspaceSkills.length > 0) parts.push(workspaceSkills);
  if (hasTool(agent, "ask_user")) parts.push(CLARIFY);
  if (hasTool(agent, "todowrite")) parts.push(TODOS);
  if (hasTool(agent, "bash")) parts.push(buildToolChoice(agent));
  const personality = agent.personality.trim();
  if (personality.length > 0) parts.push(tagged("personality", personality));
  if (RENDERING.length > 0) parts.push(RENDERING);
  if (hasTool(agent, "validate_mermaid")) parts.push(MERMAID);
  if (hasTool(agent, "page_shot")) parts.push(VISUAL);
  return parts.join("\n\n");
};

export const resolveAgentSystem = (
  key: string,
  agentContexts: AgentContext[],
  skillContexts: SkillContext[],
  t: TFunction,
): string => composeAgentSystem(resolveAgentMeta(key, agentContexts, t), skillContexts);
