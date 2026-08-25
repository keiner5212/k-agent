import { hardwareThreadCount, MAX_WORKER_CORES_AUTO } from "@/types/settings";

export type WorkerCoreLease = {
  id: string;
  label: string;
  cores: number;
  release: () => void;
};

export type WorkerCoreLeaseInfo = {
  id: string;
  label: string;
  cores: number;
};

export type WorkerCoreSnapshot = {
  hardwareThreads: number;
  configuredCores: number;
  limit: number;
  inUse: number;
  available: number;
  leases: readonly WorkerCoreLeaseInfo[];
};

type Listener = (snapshot: WorkerCoreSnapshot) => void;

let configuredCores = MAX_WORKER_CORES_AUTO;
const leases = new Map<string, WorkerCoreLeaseInfo>();
const listeners = new Set<Listener>();

export const resolveWorkerCoreLimit = (maxWorkerCores: number): number => {
  const hardware = hardwareThreadCount();
  if (maxWorkerCores <= MAX_WORKER_CORES_AUTO) return hardware;
  return Math.min(hardware, Math.max(1, Math.floor(maxWorkerCores)));
};

const inUseCores = (): number => {
  let total = 0;
  for (const lease of leases.values()) {
    total += lease.cores;
  }
  return total;
};

export const getWorkerCoreSnapshot = (): WorkerCoreSnapshot => {
  const limit = resolveWorkerCoreLimit(configuredCores);
  const inUse = inUseCores();
  return {
    hardwareThreads: hardwareThreadCount(),
    configuredCores,
    limit,
    inUse,
    available: Math.max(0, limit - inUse),
    leases: [...leases.values()],
  };
};

const notify = (): void => {
  const snapshot = getWorkerCoreSnapshot();
  for (const listener of listeners) {
    listener(snapshot);
  }
};

export const syncWorkerCoreConfig = (maxWorkerCores: number): void => {
  configuredCores = maxWorkerCores;
  notify();
};

export const subscribeWorkerCores = (listener: Listener): (() => void) => {
  listeners.add(listener);
  listener(getWorkerCoreSnapshot());
  return () => {
    listeners.delete(listener);
  };
};

export const acquireWorkerCores = (label: string, cores?: number): WorkerCoreLease => {
  const limit = resolveWorkerCoreLimit(configuredCores);
  const want = Math.min(cores ?? limit, limit);
  const id = crypto.randomUUID();
  leases.set(id, { id, label, cores: want });
  notify();
  return {
    id,
    label,
    cores: want,
    release: () => {
      leases.delete(id);
      notify();
    },
  };
};
