# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/srnnkls/satz/compare/v0.1.0...v0.2.0) - 2026-11-28

### Features

- *(query)* [**breaking**] Arithmetic, edits, recursive defines and frontend schemas
- *(markdown)* [**breaking**] Pass inline rules the character before their trigger
- *(markdown)* Register inline rules that yield directives

### Bug Fixes

- *(query)* [**breaking**] Edit whole blocks and write nothing on a failed plan
- *(markdown)* Give origins file offsets past the frontmatter

### Refactor

- Name the query language sq

### Miscellaneous Tasks

- Ship sq as a shim for satz query


### Features

- Inline rules for the Markdown frontend: `Markdown::with_inline_rule` registers an `InlineRule` whose claimed text parses into an inline directive node at the claimed source, in trees and in the documents and renderings of `Markdown::document` and `Markdown::empty_document`.
- Inline directive nodes carry a `label` attribute: the text between their brackets, or the label an inline rule claimed.

## [0.1.0](https://github.com/srnnkls/satz/releases/tag/v0.1.0) - 2026-11-26

First release: structural query and lint over text, as a Rust library (`satz`) and a CLI (`satz`).

### Features

- A typed tree of nodes with source positions, built by frontends; Markdown (with directives and YAML frontmatter) and plain text ship first.
- tq, a pattern language over the tree: captures, structure, joins, grouping, aggregates and sorting.
- Lint rules written in tq, loaded as modules and layers from `satz.toml`, `.satz/lint` and the user config directory, with severities, per-file disabling and runnable examples.
- Standard rules for duplicates, document structure, links, and lexical and semantic similarity; semantic similarity embeds with Model2Vec behind the `semantic` feature.
- A Markdown document editor that writes unedited documents back byte for byte.
- `satz query` and `satz lint` with text and JSON output, and `satz lint test`.
