# tela

*tela • web, woven cloth* (German *Gewebe*)

> Latin *tela* is the web on the loom, from *texere*, "to weave", the verb
> behind *text*. A text is a weave of parts; tela reads that weave.

## About

tela queries and lints text as structure. A frontend parses a format into a
tree of nodes, hq patterns match that tree, and lint rules written in hq
report what they find. The core knows no markup: Markdown and plain text are
its first frontends.

tela is a Go library and a CLI. [Henia](https://github.com/srnnkls/henia)
builds on it to lint agent skills.

## Status

The engine is moving out of henia; nothing is released yet.
