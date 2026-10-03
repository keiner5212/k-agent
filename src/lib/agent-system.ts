import type { TFunction } from "i18next";
import { resolveAgentMeta } from "@/lib/builtin-agents";
import type { AgentContext, AgentMeta, AgentSkillRef } from "@/types/agents";
import type { AgentsMdFile } from "@/types/agents-md";
import type { McpServer } from "@/types/mcp-servers";
import type { SkillContext } from "@/types/skills";
import { wrapSection, type PromptShape } from "@/lib/prompt-shape";

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

const tagged = (name: string, body: string, shape: PromptShape): string =>
  wrapSection(name, body, shape);

const skillLine = (name: string, description: string): string => {
  const detail = description.length > 0 ? description : name;
  return `- \`${name}\`: ${detail}`;
};

const buildAgentSkills = (
  items: { name: string; description: string }[],
  shape: PromptShape,
): string => {
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
    shape,
  );
};

const buildWorkspaceSkills = (
  skillContexts: readonly SkillContext[],
  loaded: ReadonlySet<string>,
  shape: PromptShape,
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
    shape,
  );
};

const buildFlow = (hasAgentSkills: boolean, shape: PromptShape): string =>
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
    shape,
  );

const CLARIFY = [
  "Before you act, list what you would have to assume: goal, scope, files, behavior, names, and success.",
  "Call `ask_user` for every gap. One question per gap. Put the option you would have assumed first. Leave free text on.",
  "Wait for the answer. Do not start the work, and do not pick for the user.",
].join("\n");

const TODOS = [
  "Keep the session todo list matched to the work when the task has several steps.",
  "Call `todowrite` with the full list. Each item is content, status, and priority (high, medium, or low). Do not invent ids.",
  "Keep at most one item in_progress. Mark an item completed only after that step is done.",
  "Send the list again when a step starts, finishes, or is dropped. An empty list clears it.",
].join("\n");

const VISUAL = [
  "Use `page_shot` only on a page that is already being served. One shot per review.",
  "Do not repeat it with a different host, height, or selector.",
  "A blank or identical image is a capture miss. Do not edit the page to remove a black box from a bad shot.",
  "Start a dev server with `background`. It is killed when the turn ends. Do not kill its pid. Do not use `bash` for that.",
].join("\n");

const RENDERING_BODY = [
  "Chat output is GitHub-flavored markdown.",
  "- Fenced code blocks with a language hint are syntax-highlighted.",
].join("\n");

const hasTool = (agent: AgentMeta, name: string): boolean => agent.tools.includes(name);

export type AgentRosterEntry = {
  name: string;
  purpose: string;
};

const clipPurpose = (text: string): string => {
  const flat = text.replace(/\s+/g, " ").trim();
  if (flat.length <= 160) return flat;
  return `${flat.slice(0, 159)}...`;
};

const purposeOf = (agent: AgentMeta): string => {
  const description = agent.description.trim();
  if (description.length > 0) return clipPurpose(description);
  const line = agent.personality
    .split("\n")
    .map((item) => item.trim())
    .find((item) => item.length > 0);
  return clipPurpose(line ?? "");
};

export const agentRoster = (
  contexts: readonly AgentContext[],
  currentName: string,
): AgentRosterEntry[] => {
  const out: AgentRosterEntry[] = [];
  const seen = new Set<string>();
  const skip = currentName.trim().toLowerCase();
  for (const ctx of contexts) {
    for (const agent of ctx.agents) {
      const name = agent.name.trim();
      const key = name.toLowerCase();
      if (name.length === 0 || key === skip || seen.has(key)) continue;
      seen.add(key);
      out.push({ name, purpose: purposeOf(agent) });
      if (out.length === 24) return out;
    }
  }
  return out;
};

const buildAgentRoster = (
  agent: AgentMeta,
  roster: readonly AgentRosterEntry[],
  shape: PromptShape,
): string => {
  if (!hasTool(agent, "task") || roster.length === 0) return "";
  const lines = roster.map((item) => {
    const purpose = item.purpose.length > 0 ? item.purpose : item.name;
    return `- \`${item.name}\`: ${purpose}`;
  });
  return tagged(
    "agents",
    [
      "Call `task` with `agent` set to one of these names when the job matches that purpose.",
      "A child keeps that agent's skills and personality. Its tools are the read-only subagent set, not the tools saved for main mode.",
      "The child cannot ask the user, edit the plan, write files, or run a command that changes state.",
      "If the result contains a question, resolve it: ask the user or decide, then call `task` again with that answer in the prompt.",
      "You alone ask the user, update the plan, and do the work.",
      "Do not invent a name. A one-step read or edit stays in this chat.",
      "",
      ...lines,
    ].join("\n"),
    shape,
  );
};

const buildToolChoice = (agent: AgentMeta, shape: PromptShape, readBeforeEdit: boolean): string => {
  const lines = ["Use the dedicated tool. Do not use `bash` for work another tool already does."];
  if (hasTool(agent, "list_directory")) {
    lines.push(
      "List a directory with `list_directory`. To find files by name, pass `glob` to `list_directory` (`*.rs` matches any depth). `glob` is an argument of `list_directory` and of `grep`. There is no `glob` tool. Do not use `ls`, `find`, or `tree`.",
    );
    lines.push(
      "The workspace path is in the environment section. Do not list the home directory or `/` to find the project.",
    );
  }
  if (hasTool(agent, "read")) {
    lines.push(
      "Read a file with `read`. Do not use `cat`, `head`, `tail`, or `wc`. Images, PDFs, and docx come back from `read` when the model accepts that input. Do not convert or screenshot them with `bash`.",
    );
    lines.push(
      "A user image is already on that user message. Do not grab the X display to see it again. An X11 grab of a GUI is often a black frame. That is a capture miss. Stop. Do not retry ffmpeg, xwd, or import.",
    );
  }
  if (hasTool(agent, "grep")) {
    lines.push(
      "Search file contents with `grep`. To limit which files are searched, pass `glob` to `grep` (`*.ts`, or `!*.json` to exclude). Do not run `grep` or `rg` in the shell.",
    );
  }
  if (hasTool(agent, "list_directory") || hasTool(agent, "grep")) {
    lines.push(
      "Readable hidden directories are `.github` and `.agents`. Do not read or search `node_modules`, `.git`, `target`, `dist`, `build`, `vendor`, virtualenvs, or other dependency, cache, and build directories. Those are skipped. Other names that start with `.` are skipped.",
    );
  }
  if (hasTool(agent, "lsp")) {
    lines.push(
      "Use `lsp` for a definition, references, or hover when a language server is installed.",
    );
  }
  if (hasTool(agent, "task")) {
    lines.push(
      "Use `task` for a multi-step read-only side job. The child keeps that agent's skills and personality, and only the read-only subagent tools. You ask the user, update the plan, and do the work. If the child asks a question, answer it and call `task` again.",
    );
    lines.push(
      "A `task` child cannot see chat attachments or images from `read`. Do not delegate matching a picture. Put the visible facts in the prompt, or do that work here.",
    );
  }
  if (hasTool(agent, "write")) lines.push("Create or overwrite a file with `write`.");
  if (hasTool(agent, "edit")) {
    lines.push(
      readBeforeEdit
        ? "Change text with `edit` only after `read` of that same path in this conversation. A file just written with `write` still needs `read` before `edit`. One edit sends only `filePath`, `oldString`, and `newString`. Several edits send only `edits`, and each item has its own `filePath`. Never send both shapes. Top-level `filePath` is not a parent of `edits`."
        : "Change text with `edit`. One edit sends only `filePath`, `oldString`, and `newString`. Several edits send only `edits`, and each item has its own `filePath`. Never send both shapes. Top-level `filePath` is not a parent of `edits`.",
    );
  }
  if (hasTool(agent, "apply_patch")) {
    lines.push("Change several files in one diff with `apply_patch`.");
  }
  if (hasTool(agent, "create_folder")) lines.push("Make a directory with `create_folder`.");
  if (hasTool(agent, "delete")) lines.push("Remove a file or empty directory with `delete`.");
  lines.push(
    "`bash` is for a command that must run and finish, such as install, build, test, or git. It starts in the workspace. Do not `cd` to the home directory or the app config directory to find the project.",
  );
  lines.push(
    "Install project dependencies into the project environment. Do not install packages into the system or the user site.",
  );
  if (hasTool(agent, "http_request")) {
    lines.push(
      "Call an API with `http_request`, including localhost. Do not use `curl` or `wget`.",
    );
  }
  if (hasTool(agent, "background")) {
    lines.push(
      "A process that must stay up uses `background`, not `bash`. That process is killed when the turn ends. Do not kill its pid.",
    );
  }
  return tagged("tools", lines.join("\n"), shape);
};

const sanitizeIdent = (raw: string): string => {
  let out = "";
  for (const ch of raw) {
    if (/[a-zA-Z0-9]/.test(ch)) out += ch.toLowerCase();
    else if (out.length > 0 && !out.endsWith("_")) out += "_";
  }
  const trimmed = out.replace(/^_+|_+$/g, "");
  return trimmed.length === 0 ? "x" : trimmed;
};

const mcpWire = (server: string, tool: string, used: Set<string>): string => {
  const base = `mcp_${sanitizeIdent(server)}_${sanitizeIdent(tool)}`;
  let wire = base;
  let n = 2;
  while (used.has(wire)) {
    wire = `${base}_${n}`;
    n += 1;
  }
  used.add(wire);
  return wire;
};

export const buildMcpTools = (
  servers: readonly McpServer[],
  shape: PromptShape = "xml",
): string => {
  const used = new Set<string>();
  const lines: string[] = [];
  for (const server of servers) {
    if (!server.enabled) continue;
    for (const tool of server.tools ?? []) {
      const name = tool.name.trim();
      if (name.length === 0) continue;
      const detail = tool.description?.trim() || name;
      lines.push(`- \`${mcpWire(server.name, name, used)}\`: ${detail}`);
    }
  }
  if (lines.length === 0) return "";
  return tagged(
    "mcp-tools",
    ["Enabled MCP tools. Call them by these names.", "", ...lines].join("\n"),
    shape,
  );
};

export const buildWorkspaceNotes = (
  enabled: boolean,
  content: string,
  shape: PromptShape = "xml",
): string => {
  if (!enabled) return "";
  const body = content.trim();
  const lines = [
    "Workspace memory is on. Personal notes live in `.k-agent/NOTES.md`.",
    "At the end of every turn, after the answer is ready, review that file. Save only if the list changed.",
    "",
    "1. Drop. Remove a bullet that this turn contradicted, that the user overrode, or that is no longer needed.",
    "2. Add. Add a bullet only when it will still matter on a later task, the user stated it or corrected you or repeated it, and no current bullet or AGENTS.md already says it.",
    "3. Promote. If a bullet outgrows a one-line preference and is now a standing project rule, move it into `AGENTS.md`, or into `agents.md` when that file already exists. Do not edit `CLAUDE.md` or `CONTEXT.md`. Remove the bullet from NOTES.md once it is there.",
    "4. Refuse. Do not add a one-off task, a guess, a secret, chat history, or a restatement of this request.",
    "5. Cap. At most 20 bullets, one line each. To add past the cap, merge or drop a weaker bullet first.",
    "6. Save. If NOTES.md changed, write it in this turn with `write` or `edit`. If a bullet was promoted, update the workspace instruction file in the same turn. If nothing changed, leave both files alone.",
    "",
    'A bullet is a durable workspace rule, such as "always run the formatter". Not the file edited in this turn.',
  ];
  if (body.length > 0) lines.push("", "Current notes:", body);
  else lines.push("", "The file is empty. Create it when the first bullet passes step 2.");
  return tagged("workspace-notes", lines.join("\n"), shape);
};

export const buildAgentsMdRules = (
  files: readonly AgentsMdFile[],
  shape: PromptShape = "xml",
): string => {
  const parts: string[] = [];
  const globalBody =
    files.find((item) => item.kind === "global" && item.exists)?.content.trim() ?? "";
  if (globalBody.length > 0) parts.push(tagged("global-rules", globalBody, shape));
  const workspaceBody =
    files.find((item) => item.kind === "local" && item.exists)?.content.trim() ?? "";
  if (workspaceBody.length > 0) parts.push(tagged("workspace-rules", workspaceBody, shape));
  return parts.join("\n\n");
};

export const composeAgentSystem = (
  agent: AgentMeta | null,
  skillContexts: SkillContext[],
  loadedSkillNames: readonly string[] = [],
  roster: readonly AgentRosterEntry[] = [],
  shape: PromptShape = "xml",
  readBeforeEdit = true,
): string => {
  if (!agent) return "";
  const loaded = new Set(loadedSkillNames.map((name) => name.trim()).filter(Boolean));
  const agentSkills = uniqueAgentSkills(agent.skills, skillContexts).filter(
    (item) => !loaded.has(item.name),
  );
  const parts: string[] = [];
  const flow = buildFlow(agentSkills.length > 0, shape);
  if (flow.length > 0) parts.push(flow);
  const agentSkillBlock = buildAgentSkills(agentSkills, shape);
  if (agentSkillBlock.length > 0) parts.push(agentSkillBlock);
  const workspaceSkills = buildWorkspaceSkills(skillContexts, loaded, shape);
  if (workspaceSkills.length > 0) parts.push(workspaceSkills);
  if (hasTool(agent, "ask_user")) parts.push(tagged("clarify", CLARIFY, shape));
  if (hasTool(agent, "todowrite")) parts.push(tagged("todos", TODOS, shape));
  if (hasTool(agent, "bash")) parts.push(buildToolChoice(agent, shape, readBeforeEdit));
  const agents = buildAgentRoster(agent, roster, shape);
  if (agents.length > 0) parts.push(agents);
  const personality = agent.personality.trim();
  if (personality.length > 0) parts.push(tagged("personality", personality, shape));
  parts.push(tagged("rendering", RENDERING_BODY, shape));
  if (hasTool(agent, "page_shot")) parts.push(tagged("visual-check", VISUAL, shape));
  return parts.join("\n\n");
};

export const resolveAgentSystem = (
  key: string,
  agentContexts: AgentContext[],
  skillContexts: SkillContext[],
  t: TFunction,
): string => composeAgentSystem(resolveAgentMeta(key, agentContexts, t), skillContexts);
