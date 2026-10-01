import { invoke } from "@tauri-apps/api/core";
import type { TurnCheckpoint } from "@/types/sessions";

export type RollbackOutcome = {
  path: string;
  restored: boolean;
};

type PathSpan = {
  path: string;
  beforeHash: string;
  afterHash: string;
  beforeCheckpointId: string;
  afterCheckpointId: string;
};

const spansForCheckpoints = (checkpoints: TurnCheckpoint[]): PathSpan[] => {
  const byPath = new Map<string, PathSpan>();
  for (const checkpoint of checkpoints) {
    if (!checkpoint.checkpointId) continue;
    for (const file of checkpoint.files) {
      const current = byPath.get(file.path);
      if (!current) {
        byPath.set(file.path, {
          path: file.path,
          beforeHash: file.beforeHash,
          afterHash: file.afterHash,
          beforeCheckpointId: checkpoint.checkpointId,
          afterCheckpointId: checkpoint.checkpointId,
        });
        continue;
      }
      current.afterHash = file.afterHash;
      current.afterCheckpointId = checkpoint.checkpointId;
    }
  }
  return [...byPath.values()];
};

export const rollbackCheckpoints = async (
  sessionId: string,
  checkpoints: TurnCheckpoint[],
  side: "before" | "after",
): Promise<RollbackOutcome[]> => {
  const files = spansForCheckpoints(checkpoints).map((span) =>
    side === "before"
      ? {
          path: span.path,
          checkpointId: span.beforeCheckpointId,
          side,
          expectHash: span.afterHash,
          restoreHash: span.beforeHash,
        }
      : {
          path: span.path,
          checkpointId: span.afterCheckpointId,
          side,
          expectHash: span.beforeHash,
          restoreHash: span.afterHash,
        },
  );
  if (files.length === 0) return [];
  const result = await invoke<{ files: RollbackOutcome[] }>("rollback_files", {
    input: { sessionId, files },
  });
  return result.files;
};
