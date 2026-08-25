/** Add a JobName + handleJob case for new heavy work. Do not run it on the UI thread. */
export type JobName =
  "listSkills" | "listAgents" | "listAgentsMd" | "listSystemFonts" | "estimateTokens";

export type ListBundle<T> = {
  contexts: T[];
  workspacePath: string | null;
};

export type EstimateTokensPayload = {
  text: string;
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
    default: {
      const exhaustive: never = name;
      throw new Error(`unknown job: ${String(exhaustive)}`);
    }
  }
};
