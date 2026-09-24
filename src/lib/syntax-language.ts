const KNOWN_LANGUAGES = new Set<string>([
  "bash",
  "c",
  "cpp",
  "csharp",
  "css",
  "diff",
  "dockerfile",
  "go",
  "graphql",
  "ini",
  "java",
  "javascript",
  "json",
  "kotlin",
  "less",
  "lua",
  "makefile",
  "markdown",
  "objectivec",
  "perl",
  "php",
  "plaintext",
  "python",
  "r",
  "ruby",
  "rust",
  "scala",
  "scss",
  "shell",
  "sql",
  "swift",
  "typescript",
  "xml",
  "yaml",
]);

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
  return KNOWN_LANGUAGES.has(resolved) ? resolved : null;
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
