import { useSyncExternalStore } from "react";
import {
  getWorkerCoreSnapshot,
  subscribeWorkerCores,
  type WorkerCoreSnapshot,
} from "./worker-cores";

export const useWorkerCoreSnapshot = (): WorkerCoreSnapshot =>
  useSyncExternalStore(subscribeWorkerCores, getWorkerCoreSnapshot, getWorkerCoreSnapshot);
