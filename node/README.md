# `fsmanifest`

A promise-based TypeScript API for loading, validating, and resolving fsman
manifests. A native addon built with Neon calls the Rust `fsman` crate directly,
so Node uses the same parser and filesystem resolver without launching a child
process.

## Requirements

- Node.js 20 or newer.

The published package installs a prebuilt native addon for Windows x64, macOS
x64 or ARM64, and glibc-based Linux x64 or ARM64. Building from a repository
checkout also requires a current Rust toolchain.

Install it from npm:

```sh
npm install fsmanifest
```

## Usage

```ts
import { FSMan } from "fsmanifest";

const config = await FSMan.load("./dots.fsman");
const isValid = await FSMan.validate();

if (isValid) {
  const paths = await FSMan.resolve(config, {
    cwd: process.env.HOME,
    short: true,
    flat: true,
  });
}
```

`load()` captures an immutable snapshot of the file. `validate()` accepts a
config explicitly or uses the most recently loaded config. `resolve()` defaults
`cwd` to `process.cwd()` and returns a `ResolvedTree`. With `flat: true`, its
return type is `string[]`.

The available resolution options are:

| Option | Type | Meaning |
| --- | --- | --- |
| `cwd` | `string` | Filesystem root for relative manifest paths. |
| `depth` | `number` | Maximum traversal depth for each `**` or `***`. |
| `short` | `boolean` | Collapse complete recursive selections. |
| `flat` | `boolean` | Return selected paths instead of a tree. |

## Development

```sh
cd node
npm install
npm run check
npm test
```

`npm run build` compiles a local Neon addon and the TypeScript API. Published
addons live in platform-specific optional packages; `npm pack` creates the
portable JavaScript package without embedding the local build.
