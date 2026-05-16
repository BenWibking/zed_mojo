# Mojo support for Zed

This extension adds Mojo language support to Zed:

- Tree-sitter syntax highlighting for `.mojo` files
- Mojo LSP integration through `mojo-lsp-server`

The extension starts `mojo-lsp-server` through a small Node proxy. Mojo 1.0.0b1
advertises notebook and semantic-token-delta capabilities that are not accepted
by Zed 1.2.6; the proxy removes those capabilities and forwards the rest of the
protocol unchanged.

## Requirements

Install Mojo so that `mojo-lsp-server` is available on `PATH` when Zed starts.

For Pixi projects, open Zed from the Pixi environment:

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
