export type PromptShape = "xml" | "markdown";

export const promptShapeForModel = (modelId: string): PromptShape => {
  const id = modelId.toLowerCase();
  if (
    id.includes("gpt") ||
    id.includes("codex") ||
    id.includes("gemini") ||
    id.includes("kimi") ||
    id.includes("trinity") ||
    /(^|[^a-z0-9])o[134]([^a-z0-9]|$)/.test(id)
  ) {
    return "markdown";
  }
  return "xml";
};

export const wrapSection = (name: string, body: string, shape: PromptShape): string => {
  const trimmed = body.trim();
  if (trimmed.length === 0) return "";
  if (shape === "markdown") return `# ${name}\n${trimmed}`;
  return `<${name}>\n${trimmed}\n</${name}>`;
};
