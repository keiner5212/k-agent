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

export const resolveLanguage = (
  language: string | null | undefined,
  path: string | null | undefined,
): string => normalizeLanguage(language) ?? detectLanguageFromPath(path) ?? "plaintext";
