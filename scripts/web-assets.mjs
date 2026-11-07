// SSR stylesheet URLs must resolve to files shipped in the client output.
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

const output = resolve(process.argv[2] ?? "web/.output");
const styles = new Set();
function scan(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = resolve(directory, entry.name);
    if (entry.isDirectory()) scan(file);
    else if (file.endsWith(".mjs")) {
      for (const match of readFileSync(file, "utf8").matchAll(/\/assets\/[\w.-]+\.css/g)) {
        styles.add(match[0]);
      }
    }
  }
}
scan(resolve(output, "server"));
assert(styles.size > 0, "SSR must reference a stylesheet");
for (const style of styles) {
  assert(existsSync(`${output}/public${style}`), `Missing compiled stylesheet: ${style}`);
}
console.log("compiled stylesheet references passed");
