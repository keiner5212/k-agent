import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import dockerfile from "highlight.js/lib/languages/dockerfile";
import go from "highlight.js/lib/languages/go";
import graphql from "highlight.js/lib/languages/graphql";
import ini from "highlight.js/lib/languages/ini";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import kotlin from "highlight.js/lib/languages/kotlin";
import less from "highlight.js/lib/languages/less";
import lua from "highlight.js/lib/languages/lua";
import makefile from "highlight.js/lib/languages/makefile";
import markdown from "highlight.js/lib/languages/markdown";
import objectivec from "highlight.js/lib/languages/objectivec";
import perl from "highlight.js/lib/languages/perl";
import php from "highlight.js/lib/languages/php";
import plaintext from "highlight.js/lib/languages/plaintext";
import python from "highlight.js/lib/languages/python";
import r from "highlight.js/lib/languages/r";
import ruby from "highlight.js/lib/languages/ruby";
import rust from "highlight.js/lib/languages/rust";
import scala from "highlight.js/lib/languages/scala";
import scss from "highlight.js/lib/languages/scss";
import shell from "highlight.js/lib/languages/shell";
import sql from "highlight.js/lib/languages/sql";
import swift from "highlight.js/lib/languages/swift";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

const REGISTERED = new Set<string>();

const register = (name: string, language: unknown): void => {
  if (REGISTERED.has(name)) return;
  hljs.registerLanguage(name, language as Parameters<typeof hljs.registerLanguage>[1]);
  REGISTERED.add(name);
};

register("bash", bash);
register("c", c);
register("cpp", cpp);
register("csharp", csharp);
register("css", css);
register("diff", diff);
register("dockerfile", dockerfile);
register("go", go);
register("graphql", graphql);
register("ini", ini);
register("java", java);
register("javascript", javascript);
register("json", json);
register("kotlin", kotlin);
register("less", less);
register("lua", lua);
register("makefile", makefile);
register("markdown", markdown);
register("objectivec", objectivec);
register("perl", perl);
register("php", php);
register("plaintext", plaintext);
register("python", python);
register("r", r);
register("ruby", ruby);
register("rust", rust);
register("scala", scala);
register("scss", scss);
register("shell", shell);
register("sql", sql);
register("swift", swift);
register("typescript", typescript);
register("xml", xml);
register("yaml", yaml);

const EXTENSION_LANGUAGE: Record<string, string> = {
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "javascript",
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  json: "json",
  jsonc: "json",
  json5: "json",
  css: "css",
  scss: "scss",
  sass: "scss",
  less: "less",
  html: "xml",
  htm: "xml",
  xhtml: "xml",
  xml: "xml",
  svg: "xml",
  md: "markdown",
  markdown: "markdown",
  mdx: "markdown",
  py: "python",
  pyi: "python",
  pyw: "python",
  rb: "ruby",
  rake: "ruby",
  gemspec: "ruby",
  go: "go",
  rs: "rust",
  java: "java",
  kt: "kotlin",
  kts: "kotlin",
  swift: "swift",
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  hh: "cpp",
  hxx: "cpp",
  cs: "csharp",
  csx: "csharp",
  m: "objectivec",
  mm: "objectivec",
  scala: "scala",
  sbt: "scala",
  php: "php",
  phtml: "php",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  fish: "bash",
  bat: "shell",
  cmd: "shell",
  sql: "sql",
  graphql: "graphql",
  gql: "graphql",
  gqls: "graphql",
  yaml: "yaml",
  yml: "yaml",
  toml: "ini",
  ini: "ini",
  cfg: "ini",
  conf: "ini",
  properties: "ini",
  dockerfile: "dockerfile",
  containerfile: "dockerfile",
  makefile: "makefile",
  mk: "makefile",
  gnumakefile: "makefile",
  r: "r",
  R: "r",
  lua: "lua",
  pl: "perl",
  pm: "perl",
  diff: "diff",
  patch: "diff",
  txt: "plaintext",
};

const FILENAME_LANGUAGE: Record<string, string> = {
  Dockerfile: "dockerfile",
  Containerfile: "dockerfile",
  Makefile: "makefile",
  GNUmakefile: "makefile",
  Rakefile: "ruby",
  ".bashrc": "bash",
  ".zshrc": "bash",
  ".profile": "shell",
};

const LANG_ALIASES: Record<string, string> = {
  js: "javascript",
  javascript: "javascript",
  ts: "typescript",
  typescript: "typescript",
  py: "python",
  python: "python",
  rb: "ruby",
  ruby: "ruby",
  rs: "rust",
  rust: "rust",
  sh: "bash",
  bash: "bash",
  shell: "shell",
  zsh: "bash",
  yml: "yaml",
  yaml: "yaml",
  md: "markdown",
  markdown: "markdown",
  plaintext: "plaintext",
  text: "plaintext",
  txt: "plaintext",
  htm: "xml",
  html: "xml",
  vue: "xml",
  svelte: "xml",
};

const normalizeLanguage = (language: string | null | undefined): string | null => {
  if (!language) return null;
  const key = language.toLowerCase();
  const resolved = LANG_ALIASES[key] ?? key;
  return REGISTERED.has(resolved) ? resolved : null;
};

const basenameOf = (path: string): string => {
  const normalized = path.replace(/\\/g, "/");
  const idx = normalized.lastIndexOf("/");
  return idx === -1 ? normalized : normalized.slice(idx + 1);
};

export const detectLanguageFromPath = (path: string | null | undefined): string | null => {
  if (!path) return null;
  const filename = basenameOf(path);
  if (FILENAME_LANGUAGE[filename]) return FILENAME_LANGUAGE[filename];
  const dot = filename.lastIndexOf(".");
  if (dot === -1) return null;
  const ext = filename.slice(dot + 1);
  return EXTENSION_LANGUAGE[ext] ?? null;
};

const trimSample = (source: string): string => source.trim().slice(0, 4096);

const isJsonLike = (sample: string): boolean => {
  const trimmed = sample.trim();
  if (trimmed.length === 0) return false;
  if (!(trimmed.startsWith("{") || trimmed.startsWith("["))) return false;
  try {
    JSON.parse(trimmed);
    return true;
  } catch {
    return false;
  }
};

const isXmlLike = (sample: string): boolean => {
  const trimmed = sample.trim();
  if (!trimmed.startsWith("<")) return false;
  return /<\/?[a-zA-Z][^>]*>/m.test(trimmed) || /^<!doctype/i.test(trimmed);
};

const isHtmlLike = (sample: string): boolean =>
  /^<!doctype\s+html/i.test(sample.trim()) ||
  /<html[\s>]/i.test(sample) ||
  /<head[\s>]/i.test(sample) ||
  /<body[\s>]/i.test(sample);

const isCssLike = (sample: string): boolean => {
  const trimmed = sample.trim();
  return (
    /^[\s\S]*?\{[\s\S]*?[\w-]+\s*:\s*[^;]+;[\s\S]*?\}/m.test(trimmed) ||
    /^\s*@media\b/im.test(trimmed) ||
    /^\s*\.[\w-]+\s*\{/im.test(trimmed)
  );
};

const isShellLike = (sample: string): boolean => {
  const lines = sample.split("\n").slice(0, 12);
  let hits = 0;
  for (const raw of lines) {
    const line = raw.trim();
    if (line.length === 0) continue;
    if (line.startsWith("#!")) return true;
    if (/^(export|unset|alias|if\s+\[|fi|then|do|done|function\s+\w+\s*\(\s*\)\s*\{)/.test(line))
      hits += 1;
    if (/\$\{?[\w]/.test(line)) hits += 1;
    if (/^\s*(sudo|apt|brew|pnpm|npm|yarn|cargo|git|docker|curl|wget)\s/.test(line)) hits += 1;
  }
  return hits >= 2;
};

const isYamlLike = (sample: string): boolean => {
  const lines = sample.split("\n").slice(0, 30);
  let listHits = 0;
  let keyHits = 0;
  for (const raw of lines) {
    if (/^\s*-\s+\S/.test(raw)) listHits += 1;
    if (/^\s{0,4}[\w][\w.-]*:\s+\S/.test(raw)) keyHits += 1;
  }
  // require explicit key:value pairs (with a colon followed by space), not just markdown lists
  if (keyHits >= 2 && listHits <= Math.max(2, keyHits)) {
    return true;
  }
  return false;
};

const isTomlLike = (sample: string): boolean => {
  const trimmed = sample.trim();
  return (
    /^\[\[[\w.-]+\]\]/m.test(trimmed) ||
    /^\[[\w.-]+\]\s*$/m.test(trimmed) ||
    /^[\w.-]+\s*=\s*(".*?"|true|false|\d+(\.\d+)?)\s*$/m.test(trimmed)
  );
};

const isMarkdownLike = (sample: string): boolean => {
  const lines = sample.split("\n").slice(0, 40);
  let headingHits = 0;
  let listHits = 0;
  let linkHits = 0;
  let codeFenceHits = 0;
  let blockquoteHits = 0;
  for (const raw of lines) {
    if (/^\s{0,3}#{1,6}\s+\S/.test(raw)) headingHits += 1;
    if (/^\s{0,3}[-*+]\s+\S/.test(raw)) listHits += 1;
    if (/\[.+?\]\([^)]+\)/.test(raw)) linkHits += 1;
    if (/```/.test(raw)) codeFenceHits += 1;
    if (/^\s{0,3}>\s+\S/.test(raw)) blockquoteHits += 1;
  }
  // Heading or link is a very strong markdown signal. Lists alone are not enough.
  if (headingHits >= 1 || linkHits >= 1 || blockquoteHits >= 1) return true;
  if (codeFenceHits >= 2) return true;
  if (listHits >= 3) return true;
  return false;
};

export const detectLanguageFromContent = (source: string | null | undefined): string | null => {
  if (!source || source.trim().length === 0) return null;
  const sample = trimSample(source);
  if (isJsonLike(sample)) return "json";
  if (isHtmlLike(sample)) return "xml";
  if (isXmlLike(sample)) return "xml";
  if (isCssLike(sample)) return "css";
  if (isShellLike(sample)) return "bash";
  // Markdown before YAML: markdown lists would otherwise match YAML array items.
  if (isMarkdownLike(sample)) return "markdown";
  if (isTomlLike(sample)) return "ini";
  if (isYamlLike(sample)) return "yaml";
  return null;
};

export const resolveLanguage = (
  language: string | null | undefined,
  path: string | null | undefined,
  source?: string,
): string | null =>
  normalizeLanguage(language) ?? detectLanguageFromPath(path) ?? detectLanguageFromContent(source);

const escapeHtml = (value: string): string =>
  value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");

export const highlightLine = (line: string, language: string | null): string => {
  if (line.length === 0) return "&nbsp;";
  if (!language) return escapeHtml(line);
  try {
    const result = hljs.highlight(line, {
      language,
      ignoreIllegals: true,
    });
    return result.value;
  } catch {
    return escapeHtml(line);
  }
};

export const highlightLines = (text: string, language: string | null): string[] => {
  const normalized = text.length === 0 ? [""] : text.split("\n");
  return normalized.map((line) => highlightLine(line, language));
};
