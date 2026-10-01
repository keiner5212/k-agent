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
