# `@fsman/node`

A promise-based TypeScript API for loading, validating, and resolving fsman
manifests. A native addon built with Neon calls the Rust `fsman` crate directly,
so Node uses the same parser and filesystem resolver without launching a child
process.

## Requirements

- Node.js 20 or newer.

The published package includes its native addon. Building from a repository
checkout also requires a current Rust toolchain.

## Usage

```ts
import { FSMan } from "@fsman/node";

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

`npm run build` compiles the Neon addon and TypeScript. `npm pack` builds the
addon with Cargo's release profile before creating the package tarball.
