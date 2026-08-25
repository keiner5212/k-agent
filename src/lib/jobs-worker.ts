import { handleJob, type JobName } from "./jobs-handlers";

type RunMessage = {
  kind: "run";
  id: string;
  name: JobName;
  payload?: unknown;
};

type InvokeResultMessage = {
  kind: "invokeResult";
  id: string;
  ok: boolean;
  result?: unknown;
  error?: string;
};

type InvokeWaiter = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
};

const invokeWaiters = new Map<string, InvokeWaiter>();

const host = {
  invoke: <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    const id = crypto.randomUUID();
    return new Promise<T>((resolve, reject) => {
      invokeWaiters.set(id, {
        resolve: (value) => resolve(value as T),
        reject,
      });
      postMessage({ kind: "invoke", id, cmd, args });
    });
  },
};

const onMessage = (event: MessageEvent<RunMessage | InvokeResultMessage>): void => {
  const message = event.data;
  if (message.kind === "invokeResult") {
    const waiter = invokeWaiters.get(message.id);
    if (!waiter) return;
    invokeWaiters.delete(message.id);
    if (message.ok) waiter.resolve(message.result);
    else waiter.reject(new Error(message.error ?? "invoke failed"));
    return;
  }
  if (message.kind !== "run") return;
  void handleJob(message.name, message.payload, host)
    .then((result) => {
      postMessage({ kind: "jobResult", id: message.id, ok: true, result });
    })
    .catch((error: unknown) => {
      const text = error instanceof Error ? error.message : "job failed";
      postMessage({ kind: "jobResult", id: message.id, ok: false, error: text });
    });
};

self.addEventListener("message", onMessage);
