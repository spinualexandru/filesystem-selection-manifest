import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";

const require = createRequire(import.meta.url);
const clientDirectory = resolve(dirname(require.resolve("vscode-languageclient/node")), "..");

// minimatch 11 parses patterns in its constructor and no longer exposes makeRe().
// These callers only check whether a pattern can match, which set.length preserves.
// Remove this patch once vscode-languageclient supports v11.
for (const file of ["common/fileOperations.js", "common/utils/globPattern.js"]) {
  const path = resolve(clientDirectory, file);
  const source = readFileSync(path, "utf8");
  const before = "if (!matcher.makeRe()) {";
  const after = "if (matcher.set.length === 0) {";

  if (source.includes(after) && !source.includes(before)) {
    continue;
  }

  if (source.split(before).length !== 2) {
    throw new Error(`Review the minimatch compatibility patch for ${file}: upstream code changed`);
  }

  writeFileSync(path, source.replace(before, after));
}
