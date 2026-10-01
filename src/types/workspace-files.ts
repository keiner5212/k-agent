export type WorkspaceEntryKind = "file" | "dir";

export type WorkspaceEntry = {
  path: string;
  kind: WorkspaceEntryKind;
};
