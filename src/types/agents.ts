export type AgentContextKind = "global" | "builtin";

export type AgentSkillKind = "global";

export const AGENT_TOOL_IDS = [
  "skill",
  "read",
  "write",
  "edit",
  "list_directory",
  "ask_user",
  "create_folder",
  "delete",
  "fetch_url",
  "internet_search",
  "http_request",
  "graphql",
  "page_shot",
  "todowrite",
  "validate_mermaid",
  "bash",
  "background",
  "grep",
] as const;

export type AgentToolId = (typeof AGENT_TOOL_IDS)[number];

export const PLAN_AGENT_TOOL_IDS: readonly AgentToolId[] = [
  "skill",
  "read",
  "list_directory",
  "ask_user",
  "todowrite",
  "internet_search",
  "fetch_url",
  "validate_mermaid",
  "grep",
];

export const CHAT_TOOL_DESCRIPTIONS: Record<AgentToolId, string> = {
  skill: "Load a skill by name. Returns SKILL.md body and dir.",
  read: "Read a file. Path absolute or workspace-relative. Outside the workspace, waits for the user to allow or deny. Use this instead of cat, head, tail, or wc.",
  write: "Create or overwrite a file. Outside the workspace, waits for the user to allow or deny.",
  edit: "Exact string replace in one or more files. Pass filePath, oldString, and newString for one edit, or edits for several applied together. Read first. Outside the workspace, waits for the user to allow or deny.",
  list_directory:
    "List directory entries. recursive/maxDepth optional. Outside the workspace, waits for the user to allow or deny. Use this instead of ls, find, or tree.",
  ask_user:
    "Ask the user up to 4 questions and block until they answer. Each question can have multiple selectable options plus an optional free-text input.",
  create_folder:
    "Create a directory at an absolute or workspace-relative path. Outside the workspace, waits for the user to allow or deny. Idempotent.",
  delete:
    "Delete a file or empty directory. Outside the workspace, waits for the user to allow or deny. The deleted file's line count is subtracted from the context counter.",
  fetch_url:
    "Read one public page. HTTPS by default. Public HTTP only when HTTP fetch is enabled in settings. Loopback and private hosts stay blocked. Use after internet_search, or when a URL is already known.",
  internet_search:
    "Find current public URLs. Returns titles, URLs, and short snippets only. Snippets are not the page. Call fetch_url on a chosen URL to read it. HTTPS results by default; public HTTP results appear only when HTTP fetch is enabled. Off-topic results are dropped.",
  http_request:
    "Send one HTTP request to any http or https URL, including localhost. Method, headers, query, and body are optional. GET and HEAD run immediately. Any other method waits for the user to deny, allow once, or allow for this chat.",
  graphql:
    "Send one GraphQL request. POST by default with query, optional variables, operation name, and headers. GET does not ask. POST and any other method wait for the user to deny, allow once, or allow for this chat.",
  page_shot:
    "Capture one viewport of a page that is already being served. http or https, including localhost. One shot per review. Do not retry with another host, a taller window, or a new selector when the image is blank or unchanged. A blank image is a capture miss, not the page design. Height is the window (max 1200), not the document. A URL hash scrolls that section into the window. Start a server with background. It is killed when the turn ends. Do not use bash for that.",
  todowrite:
    "Update the session todo list incrementally. Each item has a stable id. Use add for new items, update to change existing ones by id, remove to delete by id, and clear: true to wipe the list. Empty args is a no-op, not a clear. An error names the known ids. Priority is numeric 0-10. The list is shown to the user and persisted across restarts.",
  validate_mermaid:
    "Check one mermaid diagram with the same parser the chat uses. Call this before a mermaid fence goes in the reply. status ok means the source parsed. status error returns the parser message. Fix the source and call again. Do not put a failed diagram in the reply.",
  bash: "Run one shell command in the workspace and wait for it to finish. Exact blocked commands never run. Exact allowed commands skip the prompt. Destructive commands, real file redirects, and non-local network commands wait for the user. 2>&1 and redirects to /dev/null do not ask. curl or wget to localhost, 127.0.0.1, or ::1 does not ask. Do not start a dev server here. Use background for a process that must stay up until the turn ends. Do not use &, nohup, or disown. Do not list, read, search, write, edit, or delete files here. Use list_directory, read, grep, write, edit, create_folder, and delete. bash is for install, build, test, and git.",
  background:
    "Start one command and keep it until this turn ends, then kill it. Use this for a dev server or preview. Do not append &, nohup, or disown. The command is the process itself, for example npm run dev. Returns the pid and the first 1200ms of output.",
  grep: "Search file contents with ripgrep. pattern is a regex. path defaults to the workspace. glob includes only matching paths (a leading ! excludes). caseInsensitive ignores letter case. count is the number of matching lines. matches is a sample of at most 100 lines. Uses the configured worker cores. Use this instead of grep or rg in bash.",
};

export const MAX_AGENT_SKILLS = 10;
export const MAX_AGENT_PERSONALITY_LINES = 200;

export const personalityLineCount = (text: string): number => {
  if (text.length === 0) return 0;
  return text.split("\n").length;
};

export const clampPersonality = (text: string): string => {
  if (text.length === 0) return "";
  const lines = text.split("\n");
  if (lines.length <= MAX_AGENT_PERSONALITY_LINES) return text;
  return lines.slice(0, MAX_AGENT_PERSONALITY_LINES).join("\n");
};

export type AgentSkillRef = {
  kind: AgentSkillKind;
  id: string;
};

export type AgentMeta = {
  id: string;
  path: string;
  name: string;
  description: string;
  personality: string;
  estimatedTokens: number;
  skills: AgentSkillRef[];
  tools: string[];
};

export type AgentContext = {
  kind: AgentContextKind;
  path: string;
  agents: AgentMeta[];
};

export const agentKey = (kind: AgentContextKind, id: string): string => `${kind}:${id}`;

export const parseAgentKey = (value: string): { kind: AgentContextKind; id: string } | null => {
  const sep = value.indexOf(":");
  if (sep <= 0) return null;
  const kind = value.slice(0, sep);
  const id = value.slice(sep + 1);
  if ((kind !== "global" && kind !== "builtin") || id.length === 0) return null;
  return { kind, id };
};

export const skillRefKey = (skill: AgentSkillRef): string => `${skill.kind}/${skill.id}`;
