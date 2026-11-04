import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("./select-release-tag.mjs", import.meta.url));

function selectTag(tags, annotated) {
  const cwd = mkdtempSync(join(tmpdir(), "fsman-release-tags-"));
  const output = join(cwd, "output");
  const env = {
    ...process.env,
    GIT_AUTHOR_NAME: "Workflow test",
    GIT_AUTHOR_EMAIL: "workflow@example.com",
    GIT_COMMITTER_NAME: "Workflow test",
    GIT_COMMITTER_EMAIL: "workflow@example.com",
    GITHUB_OUTPUT: output,
  };
  const git = (...args) => execFileSync("git", args, { cwd, env, encoding: "utf8" }).trim();
  try {
    git("init", "--quiet");
    git("commit", "--quiet", "--allow-empty", "-m", "Release commit");
    const sha = git("rev-parse", "HEAD");
    for (const tag of tags) {
      git("tag", ...(tag === annotated ? ["-a", "-m", "Release tag"] : []), tag);
    }
    // Selecting a tag must return its commit, even if HEAD has moved ahead.
    git("commit", "--quiet", "--allow-empty", "-m", "Later commit");
    const result = spawnSync(process.execPath, [script], { cwd, env, encoding: "utf8" });
    return { ...result, sha, output: existsSync(output) ? readFileSync(output, "utf8") : "" };
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
}

test("selects the highest version rather than tag creation or lexical order", () => {
  const result = selectTag(["v0.10.0", "v0.9.9", "v0.1.4", "nightly", "v01.99.0", "v1.0.0-01"]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.output, `tag=v0.10.0\nsha=${result.sha}\n`);
});

test("orders prerelease identifiers numerically and ranks stable releases above them", () => {
  for (const [tags, expected] of [
    [["v1.0.0-rc.10", "v1.0.0-rc.2"], "v1.0.0-rc.10"],
    [["v1.0.0-rc.10", "v1.0.0"], "v1.0.0"],
    [["v0.9.0", "v1.0.0-alpha"], "v1.0.0-alpha"],
    [["v1.0.0-alpha.1", "v1.0.0-alpha.beta", "v1.0.0-beta"], "v1.0.0-beta"],
    [["v1.0.0-alpha", "v1.0.0-alpha.1"], "v1.0.0-alpha.1"],
    [["v9007199254740993.0.0", "v9007199254740992.0.0"], "v9007199254740993.0.0"],
  ]) {
    const result = selectTag(tags);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.output, `tag=${expected}\nsha=${result.sha}\n`);
  }
});

test("peels annotated release tags to their commit", () => {
  const result = selectTag(["v0.1.3", "v0.1.4"], "v0.1.4");
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.output, `tag=v0.1.4\nsha=${result.sha}\n`);
});

test("fails clearly when no valid release tag exists", () => {
  for (const tags of [[], ["nightly", "v01.0.0", "v1.0.0-01", "v1.0.0-alpha.01"]]) {
    const result = selectTag(tags);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /No release tags found/);
    assert.equal(result.output, "");
  }
});
