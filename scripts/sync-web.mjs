import { cp, rm, access } from "node:fs/promises";
import { constants } from "node:fs";

const src = "apps/web/out";
const dst = "apps/desktop/public";

try {
  await access(src, constants.F_OK);
} catch {
  console.error(`[sync] Missing ${src}. Run 'pnpm build:web' first.`);
  process.exit(1);
}

await rm(dst, { recursive: true, force: true });
await cp(src, dst, { recursive: true });
console.log(`[sync] ${src} -> ${dst}`);