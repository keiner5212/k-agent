import { afterEach, describe, expect, it } from "vitest";
import {
  acquireWorkerCores,
  getWorkerCoreSnapshot,
  resetWorkerCoreState,
  syncWorkerCoreConfig,
} from "./worker-cores";

afterEach(() => {
  resetWorkerCoreState();
});

describe("worker cores", () => {
  it("caps a single lease at the configured limit", () => {
    syncWorkerCoreConfig(2);
    const lease = acquireWorkerCores("job", 8);
    expect(lease.cores).toBe(2);
    expect(lease.cores).toBeLessThanOrEqual(getWorkerCoreSnapshot().limit);
    lease.release();
  });

  it("still grants one core when the pool is exhausted so IO can proceed", () => {
    syncWorkerCoreConfig(1);
    const first = acquireWorkerCores("a", 1);
    const second = acquireWorkerCores("b", 1);
    expect(first.cores).toBe(1);
    expect(second.cores).toBe(1);
    expect(getWorkerCoreSnapshot().inUse).toBe(2);
    first.release();
    second.release();
  });

  it("never grants more than the limit in one lease", () => {
    syncWorkerCoreConfig(3);
    const lease = acquireWorkerCores("big", 999);
    expect(lease.cores).toBe(3);
    lease.release();
  });
});
