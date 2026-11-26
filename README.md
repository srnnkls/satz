# tela

*tela • web, woven cloth* (German *Gewebe*)

> Latin *tela* is the web on the loom, from *texere*, "to weave", the verb
> behind *text*. A text is a weave of parts; tela reads that weave.

## About

tela queries and lints text as structure. A frontend parses a format into a
tree of nodes, tq patterns match that tree, and lint rules written in tq
report what they find. The core knows no markup: Markdown and plain text are
its first frontends.

tela is a Rust library and a CLI. [Henia](https://github.com/srnnkls/henia)
builds on it to lint agent skills.

## Install

```sh
cargo install --locked --git https://github.com/srnnkls/tela tela-cli
```

This installs the `tela` command. The library is the `tela` crate in the same
repository.

## Quickstart

````bash
cat > guide.md <<'EOF'
# Guide

## Install

```sh
cargo install tela-cli
```

## Install

Run `make release`.
EOF
````

Query the headings of level 2, and the code blocks by language:

```console
$ tela query '(heading :level 2) @h'
@h  guide.md#install  L3  heading  Install
@h  guide.md#install-1  L9  heading  Install
$ tela query '(code :inline false :lang ?l @lang) @c'
@lang  sh
@c  guide.md#install  L5-7  code  cargo install tela-cli
```

Lint the directory with the std rules:

```console
$ tela lint
guide.md:9:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
```

## Documentation

- [tela query](docs/query.md): the tq language, its node types, values,
  captures, grouping, output and regular expressions.
- [tela lint](docs/lint.md): `tela.toml`, inline rules, rule modules and
  their layers, similarity and JSON output.
- [Std rules](docs/rules.md): the rules tela ships.
- The library: `cargo doc -p tela --open` documents embedding tela, with
  frontends, trees, queries, lint plans and semantic models.

## Development

```sh
mise run loqui:check   # rustfmt, lint table, Clippy, tests, cargo deny
mise run cli           # the CLI contract, as scrut tests
mise run parity        # tela against the pinned henia, as scrut tests
```

## Status

The engine is moving out of henia; nothing is released yet.
