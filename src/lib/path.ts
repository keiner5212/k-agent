export const toPosixPath = (value: string): string => value.replace(/\\/g, "/");
export const trimSeparator = (value: string): string => value.replace(/^[\\/]+|[\\/]+$/g, "");
export const joinWorkspacePath = (root: string, rel: string): string => {
  const nativeSep: "/" | "\\" = root.includes("\\") ? "\\" : "/";
  const rootNormalized = nativeSep === "\\" ? root : toPosixPath(root).replace(/[\\/]+$/, "");
  const relNormalized = toPosixPath(rel).replace(/^\/+|\/+$/g, "");
  const joined =
    relNormalized.length > 0 ? `${rootNormalized}${nativeSep}${relNormalized}` : rootNormalized;
  return nativeSep === "\\" ? joined.replace(/\//g, "\\") : joined;
};
export const expandUserPath = (input: string, home: string | null): string => {
  const v = toPosixPath(input.trim());
  if (v === "~") return home ?? v;
  if (home && v.startsWith("~/")) return `${home}/${v.slice(2)}`;
  return v;
};
export const relativeToWorkspacePath = (workspaceRoot: string, path: string): string => {
  const p = toPosixPath(path);
  if (!workspaceRoot) return p;
  const r = toPosixPath(workspaceRoot).replace(/\/+$/, "");
  if (p.startsWith(r)) {
    const rest = p.slice(r.length).replace(/\/+$/, "");
    return rest || p;
  }
  return p;
};
