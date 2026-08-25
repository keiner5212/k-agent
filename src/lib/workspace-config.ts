import { useAgentsMdStore } from "@/lib/agents-md";
import { useAgentsStore } from "@/lib/agents";
import { runListWorkspaceConfigJob } from "@/lib/jobs";
import { ipcErrorMessage, isTauri } from "@/lib/platform";
import { perfLog } from "@/lib/perf-log";
import { useSkillsStore } from "@/lib/skills";
import { acquireWorkerCores } from "@/lib/worker-cores";

const CONFIG_TTL_MS = 1000;

let loadedAt = 0;
let hasHydrated = false;
let chain: Promise<void> = Promise.resolve();

const applyEmpty = (): void => {
  useSkillsStore.setState({ contexts: [], workspacePath: null, loading: false, error: undefined });
  useAgentsStore.setState({ contexts: [], workspacePath: null, loading: false, error: undefined });
  useAgentsMdStore.setState({ files: [], workspacePath: null, loading: false, error: undefined });
};

const hydrateOnce = async (force: boolean): Promise<void> => {
  if (!force && loadedAt > 0 && performance.now() - loadedAt < CONFIG_TTL_MS) {
    return;
  }
  const start = performance.now();
  if (!isTauri()) {
    applyEmpty();
    hasHydrated = true;
    loadedAt = performance.now();
    return;
  }
  if (!hasHydrated) {
    useSkillsStore.setState({ loading: true, error: undefined });
    useAgentsStore.setState({ loading: true, error: undefined });
    useAgentsMdStore.setState({ loading: true, error: undefined });
  }
  const lease = acquireWorkerCores("listWorkspaceConfig", 3);
  try {
    const bundle = await runListWorkspaceConfigJob();
    useSkillsStore.setState({
      contexts: bundle.skills,
      workspacePath: bundle.workspacePath,
      loading: false,
      error: undefined,
    });
    useAgentsStore.setState({
      contexts: bundle.agents,
      workspacePath: bundle.workspacePath,
      loading: false,
      error: undefined,
    });
    useAgentsMdStore.setState({
      files: bundle.agentsMd,
      workspacePath: bundle.workspacePath,
      loading: false,
      error: undefined,
    });
    hasHydrated = true;
    loadedAt = performance.now();
    perfLog("workspaceConfig.hydrate", performance.now() - start, {
      cores: lease.cores,
      skills: bundle.skills.length,
      agents: bundle.agents.length,
      agentsMd: bundle.agentsMd.length,
    });
  } catch (error) {
    const message = ipcErrorMessage(error);
    useSkillsStore.setState({ loading: false, error: message });
    useAgentsStore.setState({ loading: false, error: message });
    useAgentsMdStore.setState({ loading: false, error: message });
  } finally {
    lease.release();
  }
};

export const hydrateWorkspaceConfig = (force = false): Promise<void> => {
  const job = chain.then(() => hydrateOnce(force));
  chain = job.then(
    () => undefined,
    () => undefined,
  );
  return job;
};

export const invalidateWorkspaceConfig = (): void => {
  loadedAt = 0;
};
