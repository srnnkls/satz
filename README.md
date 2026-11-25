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

## Development

```sh
mise run loqui:check   # rustfmt, lint table, Clippy, tests, cargo deny
```

## Status

The engine is moving out of henia; nothing is released yet.
