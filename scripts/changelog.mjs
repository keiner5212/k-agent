import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;

const mode = process.argv[2];
if (mode !== undefined && mode !== "check" && mode !== "body") {
  console.error("Use: node scripts/changelog.mjs [check|body]");
  process.exit(1);
}

const drift = versionDrift();
if (drift.length > 0) {
  console.error(
    `The version ${version} of package.json does not match ${drift.join(", ")}. They must agree.`,
  );
  process.exit(1);
}

const changelog = readFileSync(join(root, "CHANGELOG.md"), "utf8");
const section = sectionFor(changelog, version);

if (!section) {
  console.error(
    `CHANGELOG.md has no section for ${version}. Add ## [${version}] before releasing.`,
  );
  process.exit(1);
}

const notes = section.split("\n").slice(1).join("\n").trim();

if (!notes) {
  console.error(`CHANGELOG.md section for ${version} is empty.`);
  process.exit(1);
}

if (mode === "body") {
  process.stdout.write(`${section}\n`);
}

function versionDrift() {
  const drift = [];
  const tauri = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8")).version;
  if (tauri !== version) {
    drift.push(`src-tauri/tauri.conf.json (${tauri})`);
  }
  const cargo = readFileSync(join(root, "src-tauri/Cargo.toml"), "utf8");
  const cargoVersion = /^version = "([^"]+)"/m.exec(cargo)?.[1];
  if (cargoVersion !== version) {
    drift.push(`src-tauri/Cargo.toml (${cargoVersion ?? "none"})`);
  }
  return drift;
}

function sectionFor(markdown, appVersion) {
  const lines = markdown.split(/\r?\n/);
  const heading = new RegExp(`^## \\[${escapeRegExp(appVersion)}\\](?:\\s|$)`);
  const start = lines.findIndex((line) => heading.test(line));
  if (start === -1) {
    return null;
  }
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (lines[index].startsWith("## ")) {
      end = index;
      break;
    }
  }
  return lines.slice(start, end).join("\n").trim();
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
