<p align="center">
  <img src="../assets/logo.svg" alt="fsman logo" width="128">
</p>

<h1 align="center">fsman for Visual Studio Code</h1>

Language support for [fsman](../README.md) filesystem selection manifests.
The extension recognizes `.fsman` files, adds syntax highlighting, and reports
parser errors as you type by running `fsman-lsp` over standard input/output.

## Features

- Syntax highlighting for paths, blocks, wildcards, and exclusions.
- Live diagnostics on document open and edit.
- Support for file-backed and untitled `.fsman` documents.
- A **fsman: Restart Language Server** command.
- Configurable language-server executable, arguments, and protocol tracing.

## Requirements

Install `fsman-lsp` from the repository and ensure it is on `PATH`:

```sh
cargo install --path crates/fsman-lsp
```

If VS Code cannot find the executable, set `fsman.server.path` to an absolute
path. During repository development, the server can be run through Cargo:

```json
{
  "fsman.server.path": "cargo",
  "fsman.server.arguments": ["run", "--quiet", "-p", "fsman-lsp"]
}
```

The Cargo configuration expects the opened workspace to be the fsman repository
root. Restart the server after changing its path or arguments; the extension
also restarts it automatically when those settings change.

## Settings

| Setting | Default | Purpose |
| --- | --- | --- |
| `fsman.server.path` | `fsman-lsp` | Executable name or path. |
| `fsman.server.arguments` | `[]` | Arguments passed to the server. |
| `fsman.trace.server` | `off` | Language-client trace level. |

## Development

```sh
cd vscode
npm ci
npm run check
npm run compile
```

Open the `vscode` directory in VS Code and run the **Run Extension** launch
configuration. To build an installable package, run
`npx @vscode/vsce package` from the same directory.

Syntax diagnostics come from `fsman-lsp`; the TextMate grammar is responsible
only for highlighting.
