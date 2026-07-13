<p align="center">
  <img src="assets/logo.svg" alt="fsman logo" width="160">
</p>

<h1 align="center">fsman</h1>

<p align="center">
  A small, readable manifest format for selecting files and directories.
</p>

`fsman` describes a filesystem selection as a tree. A manifest can name exact
paths, select the children of a directory, recurse through a subtree, and
exclude paths. The workspace provides a reusable Rust library, a validator and
resolver CLI, an LSP server, and editor extensions for VS Code and Zed.

## Manifest format

```fsman
.config {
  fish {
    config.fish
    functions {
      *
      !legacy.fish
    }
  }

  hypr {
    **
    !cache
  }
}

Work {
  **
  assets {
    ***
  }
}

.hidden
```

Each directive occupies its own line:

| Syntax | Meaning |
| --- | --- |
| `path` | Select an exact file or directory. |
| `path { ... }` | Descend into a directory and apply nested directives. |
| `*` | Select every immediate child of the current directory. |
| `**` | Select recursively, respecting `.gitignore` files and global Git ignores inside repositories. |
| `***` | Select recursively without applying Git ignore rules. |
| `!path` | Exclude a path and everything below it. |

Paths are relative to the current block. Absolute paths and parent traversal
with `..` are rejected during resolution. Missing literal paths are ignored,
and symbolic links are selected without being followed. Git-aware recursion
also skips repository `.git` metadata. Both recursive forms honor
`--depth <LEVELS>`.

See [`examples/basic.fsman`](examples/basic.fsman) for a larger example.

## Getting started

Build the workspace with a current Rust toolchain:

```sh
cargo build --workspace
```

Validate a manifest:

```sh
cargo run -p fsman-cli -- examples/basic.fsman
```

Resolve it against a directory and print the selected tree:

```sh
cargo run -p fsman-cli -- resolve examples/basic.fsman --cwd "$HOME"
```

The resolver also supports `--depth <LEVELS>` for bounded recursion,
`--short` for collapsed paths, `--json` for structured output, and `--flat`
for a path list. Flags can be combined, including `--json --flat`.

To install the published binaries from crates.io:

```sh
cargo install fsman-cli
cargo install fsman-lsp
```

### Node.js and TypeScript

The [`fsmanifest`](node/README.md) package provides a typed, promise-based API
backed by an in-process Neon addon:

```ts
import { FSMan } from "fsmanifest";

const config = await FSMan.load("./dots.fsman");

if (await FSMan.validate()) {
  const paths = await FSMan.resolve(config, { short: true, flat: true });
}
```

The package requires Node.js 20 or newer. See its README for resolution options
and local development commands.

## Editor support

- [Visual Studio Code](vscode/README.md) provides syntax highlighting and live
  parser diagnostics.
- [Zed](zed/README.md) registers the language and provides live parser
  diagnostics.

Both extensions launch `fsman-lsp`, which must be available on `PATH` or
configured explicitly in the editor.

## Publishing

Publishing is handled by the manually triggered `Publish release` GitHub
Actions workflow. Create and push a semantic-version tag such as `v0.1.0`, then
run the workflow for that tag from the Actions tab. The workflow verifies that
the tag matches all Cargo and npm versions before publishing anything.

The `release` GitHub environment needs `CRATES_IO_TOKEN` and `NPM_TOKEN`
secrets. The npm account must own the `@spinualexandru` scope used by the native
binary packages. Cargo and npm publishing can be enabled independently in the
workflow form.

## Workspace layout

- `crates/fsman` — typed manifest model, Pest parser, and filesystem resolver.
- `crates/fsman-cli` — command-line validation and resolution.
- `crates/fsman-lsp` — stdio language server for syntax diagnostics.
- `crates/fsman-node` — Neon bindings for the Node package.
- `node` — TypeScript API for loading, validation, and resolution.
- `vscode` and `zed` — editor integrations.
- `examples` — representative manifests.

## Development

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## License

fsman is available under the [MIT License](LICENSE).
