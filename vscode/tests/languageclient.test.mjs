import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { runInThisContext } from "node:vm";

const require = createRequire(import.meta.url);
const clientDirectory = resolve(dirname(require.resolve("vscode-languageclient/node")), "..");

// Exercise the installed language client's glob consumers with a small VS Code stub.
function loadClientModule(file, vscode = {}) {
  const path = resolve(clientDirectory, file);
  const localRequire = createRequire(path);
  const module = { exports: {} };
  const evaluate = runInThisContext(`(function(require, module, exports) {\n${readFileSync(path, "utf8")}\n})`, { filename: path });
  evaluate((name) => name === "vscode" ? vscode : localRequire(name), module, module.exports);
  return module.exports;
}

test("language client matches file globs with minimatch 11", () => {
  const { matchGlobPattern } = loadClientModule("common/utils/globPattern.js");
  for (const [pattern, path, expected] of [
    ["**/*.fsman", "/workspace/basic.fsman", true],
    ["**/*.fsman", "/workspace/basic.txt", false],
    ["**/*.{fsman,txt}", "/workspace/basic.fsman", true],
    ["C:\\workspace\\*.fsman", "C:/workspace/basic.fsman", true],
    ["", "/workspace/basic.fsman", false],
    ["#comment", "/workspace/basic.fsman", false],
  ]) {
    assert.equal(matchGlobPattern(pattern, { fsPath: path }), expected, `${pattern}: ${path}`);
  }
});

test("language client registers and filters file operations with minimatch 11", async () => {
  const { DidCreateFilesFeature } = loadClientModule("common/fileOperations.js", {
    workspace: { onDidCreateFiles: () => ({ dispose() {} }) },
  });
  const feature = new DidCreateFilesFeature({
    code2ProtocolConverter: { asDidCreateFilesParams: (event) => event },
  });
  try {
    feature.register({
      id: "fsman",
      registerOptions: {
        filters: [{ scheme: "file", pattern: { glob: "**/*.fsman", options: { ignoreCase: true } } }],
      },
    });
    const files = [
      { scheme: "file", fsPath: "/workspace/basic.fsman" },
      { scheme: "file", fsPath: "/workspace/.config/BASIC.FSMAN" },
      { scheme: "file", fsPath: "C:\\workspace\\basic.fsman" },
      { scheme: "file", fsPath: "/workspace/basic.txt" },
      { scheme: "untitled", fsPath: "/workspace/basic.fsman" },
    ];
    const result = await feature.filter({ files }, (uri) => uri);
    assert.deepEqual(result.files, files.slice(0, 3));
    assert.throws(() => feature.register({
      id: "comment",
      registerOptions: { filters: [{ pattern: { glob: "#comment" } }] },
    }), /Invalid pattern/);
  } finally {
    feature.clear();
  }
});
