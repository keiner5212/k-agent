import { invoke } from "@tauri-apps/api/core";
import { handleJob, type Host, type JobName, type ListBundle } from "./jobs-handlers";
import { isTauri } from "./platform";
import type { AgentContext } from "@/types/agents";
import type { AgentsMdFile } from "@/types/agents-md";
import type { SkillContext } from "@/types/skills";

type JobResultMessage = {
  kind: "jobResult";
  id: string;
  ok: boolean;
  result?: unknown;
  error?: string;
};

type InvokeMessage = {
  kind: "invoke";
  id: string;
  cmd: string;
  args?: Record<string, unknown>;
};

type JobWaiter = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
};

const jobWaiters = new Map<string, JobWaiter>();

const tauriHost: Host = {
  invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    if (!isTauri()) {
      throw new Error("Desktop shell required");
    }
    return args ? invoke<T>(cmd, args) : invoke<T>(cmd);
  },
};

let worker: Worker | null = null;
let workerFailed = false;

const attachWorker = (next: Worker): void => {
  next.addEventListener("message", (event: MessageEvent<JobResultMessage | InvokeMessage>) => {
    const message = event.data;
    if (message.kind === "invoke") {
      const send = (ok: boolean, result?: unknown, error?: string): void => {
        next.postMessage({ kind: "invokeResult", id: message.id, ok, result, error });
      };
      void tauriHost
        .invoke(message.cmd, message.args)
        .then((result) => send(true, result))
        .catch((error: unknown) => {
          const text = error instanceof Error ? error.message : "invoke failed";
          send(false, undefined, text);
        });
      return;
    }
    if (message.kind !== "jobResult") return;
    const waiter = jobWaiters.get(message.id);
    if (!waiter) return;
    jobWaiters.delete(message.id);
    if (message.ok) waiter.resolve(message.result);
    else waiter.reject(new Error(message.error ?? "job failed"));
  });
};

const getWorker = (): Worker | null => {
  if (workerFailed) return null;
  if (worker) return worker;
  try {
    worker = new Worker(new URL("./jobs-worker.ts", import.meta.url), { type: "module" });
    attachWorker(worker);
    return worker;
  } catch {
    workerFailed = true;
    return null;
  }
};

export const runJob = async <T>(name: JobName, payload?: unknown): Promise<T> => {
  const target = getWorker();
  if (!target) {
    return (await handleJob(name, payload, tauriHost)) as T;
  }
  const id = crypto.randomUUID();
  return new Promise<T>((resolve, reject) => {
    jobWaiters.set(id, {
      resolve: (value) => resolve(value as T),
      reject,
    });
    target.postMessage({ kind: "run", id, name, payload });
  });
};

export const runListSkillsJob = (): Promise<ListBundle<SkillContext>> => runJob("listSkills");

export const runListAgentsJob = (): Promise<ListBundle<AgentContext>> => runJob("listAgents");

export const runListAgentsMdJob = (): Promise<ListBundle<AgentsMdFile>> => runJob("listAgentsMd");

export const runListSystemFontsJob = (): Promise<string[]> => runJob("listSystemFonts");

export const runEstimateTokensJob = (text: string): Promise<number> =>
  runJob("estimateTokens", { text });
