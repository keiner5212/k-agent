export type SkillContextKind = "global" | "local";

export type SkillInfo = {
  id: string;
  path: string;
};

export type SkillContext = {
  kind: SkillContextKind;
  path: string;
  skills: SkillInfo[];
};

export type SkillMeta = {
  id: string;
  path: string;
  name: string;
  description: string;
};

export type SkillFile = {
  path: string;
  content: string;
};

export const DESKTOP_REQUIRED_SKILLS = "Desktop shell required";

export type SkillsMutationResult = {
  contexts?: SkillContext[];
  workspacePath?: string;
  error?: string;
};