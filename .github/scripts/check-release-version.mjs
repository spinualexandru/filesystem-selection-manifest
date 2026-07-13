import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const expected = process.argv[2];

if (!expected) {
  throw new Error("usage: node .github/scripts/check-release-version.mjs <version>");
}

const root = resolve(import.meta.dirname, "../..");
const workspaceManifest = readFileSync(join(root, "Cargo.toml"), "utf8");
const workspacePackage = tomlSection(workspaceManifest, "workspace.package");
const workspaceVersion = tomlString(workspacePackage, "version");
assertVersion("Cargo workspace", workspaceVersion);

for (const entry of readdirSync(join(root, "crates"), { withFileTypes: true })) {
  if (!entry.isDirectory()) {
    continue;
  }

  const manifest = readFileSync(join(root, "crates", entry.name, "Cargo.toml"), "utf8");
  const packageSection = tomlSection(manifest, "package");
  const name = tomlString(packageSection, "name");
  const version = /^version\.workspace\s*=\s*true\s*$/m.test(packageSection)
    ? workspaceVersion
    : tomlString(packageSection, "version");
  assertVersion(`Cargo package ${name}`, version);
}

const nodeDirectory = join(root, "node");
const libraryManifest = readJson(join(nodeDirectory, "package.json"));
assertVersion(`npm package ${libraryManifest.name}`, libraryManifest.version);

const platformDirectory = join(nodeDirectory, "platforms");
const platformPackages = readdirSync(platformDirectory, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => readJson(join(platformDirectory, entry.name, "package.json")));

const expectedOptionalDependencies = Object.fromEntries(
  platformPackages.map((manifest) => {
    assertVersion(`npm package ${manifest.name}`, manifest.version);
    return [manifest.name, expected];
  }),
);

if (
  JSON.stringify(sortObject(libraryManifest.optionalDependencies ?? {})) !==
  JSON.stringify(sortObject(expectedOptionalDependencies))
) {
  throw new Error("fsmanifest optionalDependencies do not match its platform packages");
}

console.log(`All Cargo and npm package versions match ${expected}.`);

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function assertVersion(label, actual) {
  if (actual !== expected) {
    throw new Error(`${label} is ${actual}; expected ${expected}`);
  }
}

function tomlSection(source, name) {
  const lines = source.split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim() === `[${name}]`);
  if (start === -1) {
    throw new Error(`TOML section [${name}] not found`);
  }
  const relativeEnd = lines.slice(start + 1).findIndex((line) => line.trimStart().startsWith("["));
  const end = relativeEnd === -1 ? lines.length : start + 1 + relativeEnd;
  return lines.slice(start + 1, end).join("\n");
}

function tomlString(section, key) {
  const match = section.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"\\s*$`, "m"));
  if (!match) {
    throw new Error(`TOML string ${key} not found`);
  }
  return match[1];
}

function sortObject(value) {
  return Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right)));
}
