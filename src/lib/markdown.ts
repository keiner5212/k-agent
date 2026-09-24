import { marked } from "marked";
import DOMPurify from "dompurify";
import { isExternalHref } from "@/lib/external-links";

marked.use({
  gfm: true,
  breaks: true,
  async: false,
});

const LINK_TAG_RE = /<a\b([^>]*)>/gi;
const HREF_RE = /href\s*=\s*("([^"]*)"|'([^']*)')/i;
const TITLE_ATTR_RE = /\btitle\s*=/i;
const mermaidBlockRe = (): RegExp =>
  /<pre>\s*<code(?:\s+[^>]*)?\sclass="[^"]*\blanguage-mermaid\b[^"]*"[^>]*>([\s\S]*?)<\/code>\s*<\/pre>/gi;

const escapeAttribute = (value: string): string =>
  value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const hashSource = (source: string): string => {
  let h = 5381;
  for (let i = 0; i < source.length; i += 1) {
    h = ((h << 5) + h + source.charCodeAt(i)) | 0;
  }
  return (h >>> 0).toString(36);
};

const replaceMermaidBlocks = (html: string): string =>
  html.replace(mermaidBlockRe(), (_match, encoded: string) => {
    const decoded = encoded
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'");
    const hash = hashSource(decoded);
    return `<div class="mermaid-placeholder" data-source="${escapeAttribute(encoded)}" data-hash="${hash}"></div>`;
  });

const addTitleToExternalLinks = (html: string, hint: string): string =>
  html.replace(LINK_TAG_RE, (match, attrs: string) => {
    if (TITLE_ATTR_RE.test(attrs)) return match;
    const hrefMatch = HREF_RE.exec(attrs);
    const href = hrefMatch?.[2] ?? hrefMatch?.[3] ?? "";
    if (!isExternalHref(href)) return match;
    return `<a${attrs} title="${escapeAttribute(hint)}">`;
  });

export const finishMarkdown = (parsedHtml: string, linkTitleHint?: string): string => {
  if (parsedHtml.length === 0) return "";
  const sanitized = DOMPurify.sanitize(parsedHtml, {
    ADD_ATTR: ["data-source", "data-hash"],
  });
  const withMermaid = replaceMermaidBlocks(sanitized);
  if (!linkTitleHint || linkTitleHint.length === 0) return withMermaid;
  return addTitleToExternalLinks(withMermaid, linkTitleHint);
};

export const renderMarkdown = (source: string, linkTitleHint?: string): string => {
  if (source.length === 0) return "";
  const raw = marked.parse(source, { async: false });
  const html = typeof raw === "string" ? raw : "";
  return finishMarkdown(html, linkTitleHint);
};
