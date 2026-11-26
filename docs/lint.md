# tela lint

`tela lint` checks Markdown and text files with rules written as
[tq](query.md) queries. Each row a rule's query matches becomes a finding.
The [std rules](rules.md) run by default, [modules](#modules) in the
project's and the user's lint directories add rules, and `tela.toml` tunes
them all and adds rules of its own.

Make a document with two problems:

```bash
mkdir -p docs
cat > docs/deploy.md <<'EOF'
# Deploy

Read the runbook first, then tag the release and push the tag to the remote.

## Usage

Run `make release`.

## Usage

Read the runbook first, then tag the release and push the tag to the remote.
EOF
```

```console
$ tela lint
docs/deploy.md:9:4: warning [duplicate-heading] heading repeated; first occurrence on line 5
docs/deploy.md:11:1: warning [duplicate-content] paragraph duplicates docs/deploy.md:3
```

Each finding prints `path:line:column: severity [rule] message`. Errors fail
the run; warnings fail it only under `--strict`:

```console
$ tela lint --strict || echo "exit $?"
docs/deploy.md:9:4: warning [duplicate-heading] heading repeated; first occurrence on line 5
docs/deploy.md:11:1: warning [duplicate-content] paragraph duplicates docs/deploy.md:3
exit 1
```

Exit status 0 is clean, 1 is findings that fail the run, and 2 is a usage or
config error, printed on stderr.

## What lint reads

`tela lint PATH...` reads the files and directories it names, and the current
directory without paths. It reads them as [`tela query`](query.md#files)
does: a directory yields the Markdown files below it, skipping hidden
entries, Git-ignored paths, `.git`, `node_modules` and `vendor`, while a file
named on the command line is always read, as plain text when no frontend
claims its extension. A file that cannot be read is left out with an
`omitted` line on stderr.

## Configuration

`tela.toml` in the current directory configures lint under `[lint]`;
`--config FILE` names another file, which must exist.

```bash
cat > tela.toml <<'EOF'
[lint]
disable = ["duplicate-content"]

[lint.config.duplicate-heading]
severity = "error"

[lint.config.outdated-reference.outdated]
runbook = "playbook"

[[lint.rules]]
id = "no-make"
query = '(code :inline true :matches /^make\b/) @c'
message = "Use the mise task, not {@c.text}."
EOF
```

```console
$ tela lint
docs/deploy.md:3:10: warning [outdated-reference] "runbook" is marked outdated; use "playbook"
docs/deploy.md:7:5: warning [no-make] Use the mise task, not make release.
docs/deploy.md:9:4: error [duplicate-heading] heading repeated; first occurrence on line 5
docs/deploy.md:11:10: warning [outdated-reference] "runbook" is marked outdated; use "playbook"
```

- `disable` turns rules off by id; `--disable a,b` adds to it, and repeats.
- `[lint.config.<id>]` sets `severity` and overrides the rule's params and
  data tables. A scalar is a param, a table or an array is a data table. Each
  is checked against the rule's own: a number param takes a number, and a
  table keeps its shape.
- `[[lint.rules]]` declares a rule inline; see [Inline rules](#inline-rules).
- `disable_key` names the frontmatter key a file turns rules off with; see
  [Turning rules off](#turning-rules-off).
- `[lint.semantic]` enables the semantic rules; see
  [Similarity](#similarity).

An unknown key, rule, param or table is an error, and an unknown rule id
suggests the nearest one:

```console
$ tela lint --disable duplicate-headings
tela: no rule duplicate-headings; did you mean duplicate-heading?
```

## Severity

A rule reports `warning` unless it says otherwise; `severity = "error"` under
`[lint.config.<id>]` or on an inline rule overrides it. Findings print
ordered by file, line, column, rule and message, each once.

## Turning rules off

A file turns rules off for itself under the key `tela.lint.disable` in its
frontmatter, as a rule id, a comma-separated string of ids, or a list. YAML
nesting works too: the key `tela: {lint: {disable: …}}` is the same key.

```bash
cat > docs/release.md <<'EOF'
---
tela.lint.disable: [duplicate-heading]
---

# Release

## Steps

## Steps
EOF
```

```console
$ tela lint docs/release.md
```

`duplicate-heading, outdated-reference` turns off both rules.
`[lint] disable_key = "lint.off"` reads another key instead.

## Inline rules

An inline rule in `tela.toml` takes:

| Key | Meaning |
|---|---|
| `id` | the rule id |
| `query` | a tq query; its captures locate the finding |
| `message` | the message, with placeholders as in [Rule keys](#rule-keys) |
| `severity` | `warning` (default) or `error` |
| `at` | the captures to locate at, as in `"@l @p"`; the first that matched wins, else the query's first capture |
| `related` | the captures of a related location, chosen the same way |
| `params` | `$param` values, as `{ min-words = 5 }` |
| `data` | data tables, as `{ banned = ["usage"] }` or a table of tables |

```bash
cat > tela.toml <<'EOF'
[[lint.rules]]
id = "short-section"
query = '(section :level 2 :words (< $min-words)) @s'
message = "{@s.title} has fewer than {$min-words} words"
params = { min-words = 5 }
EOF
```

```console
$ tela lint
docs/deploy.md:5:1: warning [short-section] Usage has fewer than 5 words
docs/deploy.md:9:4: warning [duplicate-heading] heading repeated; first occurrence on line 5
docs/deploy.md:11:1: warning [duplicate-content] paragraph duplicates docs/deploy.md:3
docs/release.md:7:1: warning [short-section] Steps has fewer than 5 words
docs/release.md:9:1: warning [short-section] Steps has fewer than 5 words
```

An inline rule holds one query; `(define …)` and `(import …)` belong to
modules. A query that does not read points into the rule's query:

```console
$ printf '[[lint.rules]]\nid = "bad"\nquery = "(headng) @h"\nmessage = "m"\n' > tela.toml
$ tela lint
tela: module "tela.toml", rule bad: unknown type "headng"
  (headng) @h
   ^
  did you mean heading?
```

## Modules

A module is a Markdown file of rules, the form tela's std rules take:

````md
---
data:
  banned: [usage]
---

## repeated-usage

A document keeps a single Usage section.

```tq
(import std/structure)

(rule repeated-usage
  :message "{@repeat.text} repeats; first on line {@first.line}"
  :at @repeat
  (repeated-heading @first @repeat ?text)
  (row :table banned :value ?text))
```

### Matches

```md
## Usage

## Usage
```

### Passes

```md
## Setup

## Setup
```
````

- The frontmatter sets `description`, `params` and `data` for every rule in
  the module. Any other key is an error, unless the program loading the
  module declares it.
- Each `## <rule-id>` section explains its rule in prose, defines it in a
  ```` ```tq ```` block, and tests it with ```` ```md ```` examples under
  `### Matches` and `### Passes`. A Matches example must make its rule
  report; a Passes example must not.
- An example overrides params and data in its fence info, as in
  ```` ```md min-words=3 ```` or ```` ```md outdated.legacy=current ````.
- The module name is its path relative to its layer, without `.md`, as in
  `team/headings`, under the layer's package name when it has one.
- Blocks fenced ```` ```hq ````, the language's earlier name, read the same
  as ```` ```tq ```` blocks.

Modules come in layers, each a directory of modules. `tela lint` reads two
after tela's std modules, each when it exists:

- the user's, `$XDG_CONFIG_HOME/tela/lint`, else `~/.config/tela/lint`, on
  every platform;
- the project's, `.tela/lint` in the nearest directory above the current one
  that holds `.git`, else in the current directory.

A program embedding tela passes its own layers; see the crate documentation.
Rule ids and module names are unique across all layers, and nothing shadows
anything: to change a std rule, configure it, or disable it and compose a
replacement.

### Forms

- `(rule id :key value ... pattern...)` defines a rule: a query whose rows
  become findings. One module may give an id several clauses, each with its
  own keys.
- `(define (name ?var @capture ...) pattern...)` names a pattern. Callers
  pass the variables and captures it binds; every other variable stays local
  to each use.
- `(import module)` brings in a module's defines, as `(import std/structure)`
  brings in `repeated-heading`. Names that would collide are an error.

Frontmatter `params` scalars become `$name` values. `data` maps and lists
become `row` nodes, outside any file, with `:table`, `:key` for a map or
`:index` for a list, and `:value`. `(row :table banned :value ?text)` above
joins each banned title with the headings that repeat it.

### Rule keys

| Key | Meaning |
|---|---|
| `:severity` | `warning` (default) or `error` |
| `:message` | text with `{@c}` (path:line), `{@c.line}`, `{@c.column}`, `{@c.text}`, `{@c.lines}`, `{@c.KEY}`, a value capture `{@n}`, `{@n.percent}` or `{@n.3}`, `{?var}`, `{$param}` and `{shared}` |
| `:at @a @b` | location: the first of these captures that matched; else the query's first capture |
| `:related @c ...` | related location, chosen the same way |
| `:focus ?var` | moves the location to the variable's first occurrence in the node's text, matched case-insensitively as `:contains` matches |
| `:score @capture` | fills `similarity` from a score capture |
| `:method name` | `jaccard`, `containment` or `cosine`; fills `method`, and a `:score` needs it |
| `:shared @a @b` | fills `shared_phrases` and `{shared}` with up to five shared phrases |

A `{?var}` counts as a use of the variable, and rows that print different
values stay separate findings.

### Grouped rules

A rule may end with a [`(group ...)`](query.md#grouping) clause, whose
filters take `$params`. This one flags files with more code blocks than
`max-code`:

````md
---
params:
  max-code: 1
---

## code-heavy

A file holds at most `max-code` code blocks.

```tq
(rule code-heavy
  :message "{@f.path} has {@n} code blocks; at most {$max-code}"
  :at @f
  (file (code :inline false)? @c) @f
  (group @f (count @c @n (> $max-code))))
```
````

Grouped rows feed `:at`, `:related`, `{@n}` and `{?var}` unchanged, and a
collected capture such as `@c` locates at its first node.

### Table rules

A rule can pair each `data` row with each document and keep the pairs that a
top-level [`(not ...)`](query.md#joins) misses. This one reports each axis a
Conventions table leaves out:

````md
---
data:
  axes: [Errors, Effects]
---

## missing-axis

A Conventions table has a row per axis.

```tq
(rule missing-axis
  :message "Conventions has no {?axis} row"
  :at @s
  (row :table axes :value ?axis)
  (file :path ?p (section :title "Conventions") @s)
  (not (file :path ?p (section :title "Conventions"
    (table (row (cell :column "Axis" :text ?axis)))))))
```
````

Over a file whose Conventions table holds only an Errors row and two code
blocks, the modules above report:

```text
notes/errors.md:1:1: warning [code-heavy] notes/errors.md has 2 code blocks; at most 1
notes/errors.md:1:1: warning [missing-axis] Conventions has no Effects row
```

## Similarity

`similar-content` and `semantic-content` compare each paragraph of at least
`min-words` words with the earliest earlier paragraph above a threshold.
`duplicate-content` reports exact copies, which the similarity rules leave
out. Both rules stay off until a threshold is set:

```toml
[lint.config.similar-content]
similarity = 0.3   # word-shingle Jaccard
containment = 0.9  # shared shingles over the shorter paragraph

[lint.semantic]
enabled = true
model_path = "/path/to/potion-base-2M"

[lint.config.semantic-content]
threshold = 0.5    # cosine; calibrate against accepted and rejected pairs
```

`model_path` names a local [Model2Vec](https://github.com/MinishLab/model2vec)
directory holding `config.json`, `tokenizer.json` and `model.safetensors`.
`$VAR` and `${VAR}` expand first, an unset variable to nothing, then a
leading `~/` to the home directory, and a relative path resolves against the
directory of the config file. tela downloads no models. Enabling the semantic
rules without a `model_path`, or with one that names no directory, is a
config error, and a model that fails to load is an error naming it.
`tela query --model DIR` names the model for `(similar …)`, else it reads the
same `model_path`.

`semantic-content` reports only what `similar-content` misses: where
`similar-content` reports a paragraph under its own config, the
`semantic-content` finding at that paragraph is left out. More generally,
within one module a finding scored by cosine is left out where a finding
scored by `jaccard` or `containment` points at the same place.

```bash
cat > docs/review.md <<'EOF'
# Review

Inspect every changed function and report concrete failures with enough context to reproduce the problem.

## Checklist

Inspect every changed function and report each concrete failure with enough context to reproduce it.
EOF
cat > tela.toml <<'EOF'
[lint.config.similar-content]
similarity = 0.3
EOF
```

```console
$ tela lint docs/review.md
docs/review.md:7:1: warning [similar-content] paragraph has 36.8% word-shingle jaccard with docs/review.md:3; shared phrases: changed function and; context to reproduce; enough context to; every changed function; function and report
```

## JSON output

`--format json` prints the findings as an array, `[]` when there are none.
Each finding has `path`, `line`, `column`, `severity`, `rule` and `message`,
and `related` locations when the rule names them. A similarity finding adds
`similarity`, the score, and `method`; a lexical one adds `shared_phrases`,
and a semantic one `model`.

```console
$ tela lint docs/review.md --format json
[
  {
    "path": "docs/review.md",
    "line": 7,
    "column": 1,
    "severity": "warning",
    "rule": "similar-content",
    "message": "paragraph has 36.8% word-shingle jaccard with docs/review.md:3; shared phrases: changed function and; context to reproduce; enough context to; every changed function; function and report",
    "related": [
      {
        "path": "docs/review.md",
        "line": 3,
        "column": 1
      }
    ],
    "similarity": 0.3684210526315789,
    "method": "jaccard",
    "shared_phrases": [
      "changed function and",
      "context to reproduce",
      "enough context to",
      "every changed function",
      "function and report"
    ]
  }
]
```

## Examples

`tela lint test` runs the Matches and Passes examples of every rule, std
and layered, each as a file of its own, and prints the examples that fail:

```console
$ tela lint test
8 lint examples passed
```
