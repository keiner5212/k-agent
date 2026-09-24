/** Add a JobName + handleJob case for new heavy work. Do not run it on the UI thread. */
import type { AgentContext } from "@/types/agents";
import type { AgentsMdFile } from "@/types/agents-md";
import type { SkillContext } from "@/types/skills";
import type { WorkspaceEntry } from "@/types/workspace-files";

export type JobName =
  | "listSkills"
  | "listAgents"
  | "listAgentsMd"
  | "listSystemFonts"
  | "estimateTokens"
  | "listLanguageServers"
  | "installLanguageServer"
  | "uninstallLanguageServer"
  | "resolveLanguageServer"
  | "lspRequest"
  | "listWorkspaceFiles"
  | "listWorkspaceConfig"
  | "renderMarkdown"
  | "highlightLines"
  | "diffLines";

export type ListBundle<T> = {
  contexts: T[];
  workspacePath: string | null;
};

export type EstimateTokensPayload = {
  text: string;
};

export type InstallLanguageServerPayload = {
  id: string;
};

export type ResolveLanguageServerPayload = {
  path: string;
};

export type LspRequestPayload = {
  path: string;
  method: string;
  params?: unknown;
};

export type ListWorkspaceFilesPayload = {
  relativeDir: string;
};

export type RenderMarkdownPayload = {
  source: string;
  linkTitleHint?: string;
};

export type HighlightLinesPayload = {
  text: string;
  language: string | null;
};

export type DiffLinesPayload = {
  before: string;
  after: string;
  context?: number;
};

export type Timed<T> = {
  value: T;
  computeMs: number;
};

export type WorkspaceConfigBundle = {
  skills: SkillContext[];
  agents: AgentContext[];
  agentsMd: AgentsMdFile[];
  workspacePath: string | null;
};

export type Host = {
  invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
};

export const uniqueSortedNames = (names: string[]): string[] => {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of names) {
    const name = raw.trim();
    if (name.length === 0 || name.startsWith(".")) continue;
    const key = name.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(name);
  }
  out.sort((a, b) => a.localeCompare(b));
  return out;
};

export const estimateTokensFromText = (text: string): number =>
  Math.ceil(Math.max(0, text.length) / 4);

export const handleJob = async (name: JobName, payload: unknown, host: Host): Promise<unknown> => {
  switch (name) {
    case "listSkills": {
      const [contexts, workspacePath] = await Promise.all([
        host.invoke("list_skills"),
        host.invoke("get_workspace_path"),
      ]);
      return { contexts, workspacePath };
    }
    case "listAgents": {
      const [contexts, workspacePath] = await Promise.all([
        host.invoke("list_agents"),
        host.invoke("get_workspace_path"),
      ]);
      return { contexts, workspacePath };
    }
    case "listAgentsMd": {
      const [contexts, workspacePath] = await Promise.all([
        host.invoke("list_agents_md"),
        host.invoke("get_workspace_path"),
      ]);
      return { contexts, workspacePath };
    }
    case "listSystemFonts": {
      const names = await host.invoke<string[]>("list_system_fonts");
      return uniqueSortedNames(names);
    }
    case "estimateTokens": {
      const text =
        payload && typeof payload === "object" && "text" in payload
          ? String((payload as EstimateTokensPayload).text)
          : "";
      return estimateTokensFromText(text);
    }
    case "listLanguageServers": {
      return host.invoke("list_language_servers");
    }
    case "installLanguageServer": {
      const id =
        payload && typeof payload === "object" && "id" in payload
          ? String((payload as InstallLanguageServerPayload).id)
          : "";
      return host.invoke("install_language_server", { id });
    }
    case "uninstallLanguageServer": {
      const id =
        payload && typeof payload === "object" && "id" in payload
          ? String((payload as InstallLanguageServerPayload).id)
          : "";
      return host.invoke("uninstall_language_server", { id });
    }
    case "resolveLanguageServer": {
      const path =
        payload && typeof payload === "object" && "path" in payload
          ? String((payload as ResolveLanguageServerPayload).path)
          : "";
      return host.invoke("resolve_language_server", { path });
    }
    case "lspRequest": {
      const body =
        payload && typeof payload === "object"
          ? (payload as LspRequestPayload)
          : { path: "", method: "" };
      return host.invoke("lsp_request", {
        path: String(body.path ?? ""),
        method: String(body.method ?? ""),
        params: body.params ?? null,
      });
    }
    case "listWorkspaceFiles": {
      const relativeDir =
        payload && typeof payload === "object" && "relativeDir" in payload
          ? String((payload as ListWorkspaceFilesPayload).relativeDir)
          : "";
      return host.invoke<WorkspaceEntry[]>("list_workspace_files", { relativeDir });
    }
    case "listWorkspaceConfig": {
      const [skills, agents, agentsMd, workspacePath] = await Promise.all([
        host.invoke("list_skills"),
        host.invoke("list_agents"),
        host.invoke("list_agents_md"),
        host.invoke("get_workspace_path"),
      ]);
      return { skills, agents, agentsMd, workspacePath };
    }
    case "renderMarkdown": {
      const body =
        payload && typeof payload === "object"
          ? (payload as RenderMarkdownPayload)
          : { source: "" };
      const { parseMarkdownOffThread } = await import("./markdown-offthread");
      const started = performance.now();
      const html = parseMarkdownOffThread(String(body.source ?? ""));
      return { value: html, computeMs: performance.now() - started } satisfies Timed<string>;
    }
    case "highlightLines": {
      const body =
        payload && typeof payload === "object"
          ? (payload as HighlightLinesPayload)
          : { text: "", language: null };
      const { highlightLines } = await import("./syntax-highlight");
      const started = performance.now();
      const lines = highlightLines(String(body.text ?? ""), body.language);
      return { value: lines, computeMs: performance.now() - started } satisfies Timed<string[]>;
    }
    case "diffLines": {
      const body =
        payload && typeof payload === "object"
          ? (payload as DiffLinesPayload)
          : { before: "", after: "" };
      const { diffEditorValue } = await import("./session-diff");
      const started = performance.now();
      const value = diffEditorValue(
        String(body.before ?? ""),
        String(body.after ?? ""),
        body.context,
      );
      return { value, computeMs: performance.now() - started };
    }
    default: {
      const exhaustive: never = name;
      throw new Error(`unknown job: ${String(exhaustive)}`);
    }
  }
};
