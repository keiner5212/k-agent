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

const preBlockRe = (): RegExp => /<pre\b[^>]*>[\s\S]*?<\/pre>/gi;

const CODE_COPY_ICON = `<svg class="md-code__glyph md-code__glyph--copy" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`;

const CODE_CHECK_ICON = `<svg class="md-code__glyph md-code__glyph--check" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>`;

export const withCodeCopy = (html: string, label: string): string => {
  if (!html.includes("<pre")) return html;
  const safe = escapeAttribute(label);
  return html.replace(
    preBlockRe(),
    (block) =>
      `<div class="md-code"><button type="button" class="md-code__copy" data-copy-code aria-label="${safe}" title="${safe}">${CODE_COPY_ICON}${CODE_CHECK_ICON}</button>${block}</div>`,
  );
};

export const finishMarkdown = (parsedHtml: string, linkTitleHint?: string): string => {
  if (parsedHtml.length === 0) return "";
  const sanitized = DOMPurify.sanitize(parsedHtml);
  if (!linkTitleHint || linkTitleHint.length === 0) return sanitized;
  return addTitleToExternalLinks(sanitized, linkTitleHint);
};

export const renderMarkdown = (source: string, linkTitleHint?: string): string => {
  if (source.length === 0) return "";
  const raw = marked.parse(source, { async: false });
  const html = typeof raw === "string" ? raw : "";
  return finishMarkdown(html, linkTitleHint);
};
