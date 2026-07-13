import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";

import { FSMan, FSManError } from "./index.js";

let directory: string;

before(async () => {
  directory = await mkdtemp(join(tmpdir(), "fsman-node-"));
});

after(async () => {
  await rm(directory, { recursive: true, force: true });
});

test("supports the documented load, validate, and flat resolve flow", async () => {
  await writeFile(join(directory, "keep.txt"), "keep");
  await writeFile(join(directory, "skip.txt"), "skip");
  const manifestPath = join(directory, "dots.fsman");
  await writeFile(manifestPath, "*\n!dots.fsman\n!skip.txt\n");

  const config = await FSMan.load(manifestPath);
  const isValid = await FSMan.validate();
  const paths = await FSMan.resolve(config, { cwd: directory, short: true, flat: true });

  assert.equal(isValid, true);
  assert.deepEqual(paths, [join(directory, "keep.txt")]);
});

test("returns false for invalid syntax", async () => {
  const manifestPath = join(directory, "invalid.fsman");
  await writeFile(manifestPath, "folder {\n  file.txt\n");
  const config = await FSMan.load(manifestPath);

  assert.equal(await FSMan.validate(config), false);
  await assert.rejects(
    FSMan.resolve(config, { cwd: directory }),
    (error: unknown) => error instanceof FSManError && error.message.length > 0,
  );
});

test("uses the loaded snapshot if the source file changes", async () => {
  const manifestPath = join(directory, "snapshot.fsman");
  await writeFile(manifestPath, "keep.txt\n");
  const config = await FSMan.load(manifestPath);
  await writeFile(manifestPath, "invalid {\n");

  assert.equal(await FSMan.validate(config), true);
  assert.deepEqual(
    await FSMan.resolve(config, { cwd: directory, flat: true }),
    [join(directory, "keep.txt")],
  );
});

test("returns a typed hierarchy by default", async () => {
  const manifestPath = join(directory, "tree.fsman");
  await writeFile(manifestPath, "folder {\n  nested.txt\n}\n");
  await mkdir(join(directory, "folder"), { recursive: true });
  await writeFile(join(directory, "folder", "nested.txt"), "nested");

  const config = await FSMan.load(manifestPath);
  const tree = await FSMan.resolve(config, { cwd: directory });

  assert.equal(tree.type, "directory");
  assert.equal(tree.children[0]?.path, join(directory, "folder"));
  assert.equal(tree.children[0]?.children[0]?.path, join(directory, "folder", "nested.txt"));
});

test("rejects an invalid recursive depth before starting fsman", async () => {
  const manifestPath = join(directory, "depth.fsman");
  await writeFile(manifestPath, "**\n");
  const config = await FSMan.load(manifestPath);

  await assert.rejects(FSMan.resolve(config, { depth: -1 }), RangeError);
});
