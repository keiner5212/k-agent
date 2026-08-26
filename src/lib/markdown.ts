import { marked } from "marked";
import DOMPurify from "dompurify";

marked.use({
  gfm: true,
  breaks: true,
  async: false,
});

export const renderMarkdown = (source: string): string => {
  if (source.length === 0) return "";
  const raw = marked.parse(source, { async: false });
  const html = typeof raw === "string" ? raw : "";
  return DOMPurify.sanitize(html);
};
