<p align="center">
  <img src="../assets/logo.svg" alt="fsman logo" width="128">
</p>

<h1 align="center">fsman for Zed</h1>

Language support for [fsman](../README.md) filesystem selection manifests.
The extension recognizes `.fsman` files and reports parser errors as you type
by running `fsman-lsp` over standard input/output.

## Features

- `.fsman` language registration with two-space indentation.
- Automatic brace pairing for nested manifest blocks.
- Live syntax diagnostics from the fsman parser.
- Configurable language-server executable, arguments, and environment.

## Requirements

Install `fsman-lsp` from the repository and ensure it is on `PATH`:

```sh
cargo install --path crates/fsman-lsp
```

If Zed cannot find the executable, configure it under `lsp.fsman-lsp.binary` in
`settings.json`. During repository development, the server can run through
Cargo:

```json
{
  "lsp": {
    "fsman-lsp": {
      "binary": {
        "path": "cargo",
        "arguments": ["run", "--quiet", "-p", "fsman-lsp"]
      }
    }
  }
}
```

The Cargo configuration expects the fsman repository to be in the active
worktree. An absolute path to `fsman-lsp` is more convenient for general use.

## Development

Install the WebAssembly target used by Zed extensions:

```sh
rustup target add wasm32-wasip1
```

Then open the command palette in Zed, run **zed: install dev extension**, and
select this `zed` directory. Use **zed: rebuild dev extension** after changing
the Rust source or extension metadata.

`fsman-lsp` is the source of truth for syntax diagnostics. Until fsman has a
dedicated Tree-sitter grammar, the extension uses a permissive fallback grammar
to satisfy Zed's language registration requirement; highlighting is therefore
limited.

Before publishing the extension, replace the placeholder `repository` URL in
`extension.toml` with this project's canonical GitHub URL.
