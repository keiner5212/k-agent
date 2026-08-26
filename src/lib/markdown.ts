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

const escapeAttribute = (value: string): string =>
  value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const addTitleToExternalLinks = (html: string, hint: string): string =>
  html.replace(LINK_TAG_RE, (match, attrs: string) => {
    if (TITLE_ATTR_RE.test(attrs)) return match;
    const hrefMatch = HREF_RE.exec(attrs);
    const href = hrefMatch?.[2] ?? hrefMatch?.[3] ?? "";
    if (!isExternalHref(href)) return match;
    return `<a${attrs} title="${escapeAttribute(hint)}">`;
  });

export const renderMarkdown = (source: string, linkTitleHint?: string): string => {
  if (source.length === 0) return "";
  const raw = marked.parse(source, { async: false });
  const html = typeof raw === "string" ? raw : "";
  const sanitized = DOMPurify.sanitize(html);
  if (!linkTitleHint || linkTitleHint.length === 0) return sanitized;
  return addTitleToExternalLinks(sanitized, linkTitleHint);
};
