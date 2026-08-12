# Mojo support for Zed

This extension adds Mojo language support to Zed:

- Tree-sitter syntax highlighting for `.mojo` files
- Mojo LSP integration through `mojo-lsp-server`

The extension starts `mojo-lsp-server` through a small Node proxy. Mojo 1.0.0
advertises notebook synchronization, which Zed does not support; the proxy
removes that capability and forwards the rest of the protocol unchanged,
including semantic-token delta support.

## Requirements

Install Mojo so that `mojo-lsp-server` is available in one of these locations,
in lookup order:

1. The path configured in Zed's `lsp.mojo-lsp-server.binary.path` setting
2. The worktree shell's `PATH`
3. The worktree's default Pixi environment under `.pixi/envs/default`

For example, a custom installation can be configured in Zed's `settings.json`:

```json
{
  "lsp": {
    "mojo-lsp-server": {
      "binary": {
        "path": "/path/to/mojo-lsp-server"
      }
    }
  }
}
```

Projects with a local default Pixi environment can be opened normally. To use a
named Pixi environment, launch Zed from that environment or configure its
`mojo-lsp-server` path explicitly:

```sh
pixi shell
zed .
```

## Local development

In Zed, run `zed: install dev extension` and select this repository.

After changing extension Rust code, reinstall or reload the dev extension so Zed
rebuilds `extension.wasm`.

If Zed crashes while opening a `.mojo` file, run `zed: open log` and check for
Mojo Tree-sitter query errors. Optional query files such as `brackets.scm` and
`indents.scm` should be omitted unless they contain the required captures.

If LSP features do not start, check whether Zed can find `mojo-lsp-server`:

```sh
which mojo-lsp-server
```
