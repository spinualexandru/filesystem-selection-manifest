import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";

const tags = execFileSync("git", ["tag", "--list"], { encoding: "utf8" })
  .trim()
  .split("\n")
  .map((tag) => {
    const match = tag.match(/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z.-]+))?$/);
    if (!match) return undefined;
    const prerelease = match[4]?.split(".") ?? [];
    if (!prerelease.every((part) => /^(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)$/.test(part))) {
      return undefined;
    }
    return { tag, version: match.slice(1, 4).map(BigInt), prerelease };
  })
  .filter(Boolean)
  .sort(compareVersions);

const latest = tags.at(-1);
if (!latest) {
  throw new Error("No release tags found. Create and push a semantic-version tag such as v0.1.4.");
}

const sha = execFileSync("git", ["rev-parse", `refs/tags/${latest.tag}^{commit}`], { encoding: "utf8" }).trim();
appendFileSync(process.env.GITHUB_OUTPUT, `tag=${latest.tag}\nsha=${sha}\n`);
console.log(`Selected latest release tag ${latest.tag} (${sha}).`);

function compareVersions(left, right) {
  for (let index = 0; index < 3; index++) {
    const result = compare(left.version[index], right.version[index]);
    if (result) return result;
  }
  if (!left.prerelease.length || !right.prerelease.length) {
    return compare(Number(!left.prerelease.length), Number(!right.prerelease.length));
  }
  for (let index = 0; index < Math.min(left.prerelease.length, right.prerelease.length); index++) {
    const a = left.prerelease[index];
    const b = right.prerelease[index];
    const aNumeric = /^\d+$/.test(a);
    const bNumeric = /^\d+$/.test(b);
    const result = aNumeric && bNumeric ? compare(BigInt(a), BigInt(b))
      : aNumeric !== bNumeric ? (aNumeric ? -1 : 1) : compare(a, b);
    if (result) return result;
  }
  return compare(left.prerelease.length, right.prerelease.length);
}

function compare(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}
