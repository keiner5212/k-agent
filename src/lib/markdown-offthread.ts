import { marked } from "marked";

marked.use({
  gfm: true,
  breaks: true,
  async: false,
});

export const parseMarkdownOffThread = (source: string): string => {
  if (source.length === 0) return "";
  const raw = marked.parse(source, { async: false });
  return typeof raw === "string" ? raw : "";
};
