# satz

*satz • sentence, proposition, typeset text*

> German *Satz* is a sentence, a proposition, and the setting of type: text
> given structure. satz reads that structure and what it says.

## About

satz queries and lints text as structure. A frontend parses a format into a
tree of nodes, tq patterns match that tree, and lint rules written in tq
report what they find. The core knows no markup: Markdown and plain text are
its first frontends.

satz is a Rust library and a CLI. [Henia](https://github.com/srnnkls/henia)
builds on it to lint agent skills.

## Install

```sh
cargo install --locked --git https://github.com/srnnkls/satz satz-cli
```

This installs the `satz` command. The library is the `satz` crate in the same
repository.

## Quickstart

````bash
cat > guide.md <<'EOF'
# Guide

## Install

```sh
cargo install satz-cli
```

## Install

Run `make release`.
EOF
````

Query the headings of level 2, and the code blocks by language:

```console
$ satz query '(heading :level 2) @h'
@h  guide.md#install  L3  heading  Install
@h  guide.md#install-1  L9  heading  Install
$ satz query '(code :inline false :lang ?l @lang) @c'
@lang  sh
@c  guide.md#install  L5-7  code  cargo install satz-cli
```

Lint the directory with the std rules:

```console
$ satz lint
guide.md:9:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
```

## Documentation

- [satz query](docs/query.md): the tq language, its node types, values,
  captures, grouping, output and regular expressions.
- [satz lint](docs/lint.md): `satz.toml`, inline rules, rule modules and
  their layers, similarity and JSON output.
- [Std rules](docs/rules.md): the rules satz ships.
- The library: `cargo doc -p satz --open` documents embedding satz, with
  frontends, trees, queries, lint plans and semantic models.

## Development

```sh
mise run loqui:check   # rustfmt, lint table, Clippy, tests, cargo deny
mise run cli           # the CLI contract, as scrut tests
mise run parity        # satz against the pinned henia, as scrut tests
```

## Status

The engine is moving out of henia; nothing is released yet.
