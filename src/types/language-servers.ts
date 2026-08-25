export type InstallMethod = "npm" | "go" | "pip" | "github";

export type InstallArchive = "none" | "gz" | "zip" | "tar.gz";

export type LanguageServerInstall = {
  method: InstallMethod;
  packages?: string[];
  module?: string;
  dir?: string;
  repo?: string;
  assetContains?: Record<string, string>;
  archive?: InstallArchive;
  binPath?: string;
};

export type LanguageServerSpec = {
  id: string;
  name: string;
  languageIds: string[];
  extensions: string[];
  filenames: string[];
  rootMarkers: string[];
  command: string;
  args: string[];
  env?: Record<string, string>;
  initializationOptions?: unknown;
  settings?: unknown;
  requires: string[];
  install: LanguageServerInstall;
};

export type LanguageServerRow = LanguageServerSpec & {
  installed: boolean;
  commandPath: string | null;
  missingRequires: string[];
};

export type ResolvedLanguageServer = LanguageServerRow & {
  languageId: string;
  root: string;
};
