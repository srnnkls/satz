# satz query

`satz query PATTERN` matches a sq pattern against Markdown and text files and
prints one row per match, with one line per capture. sq patterns are
S-expressions over the document tree: a `?variable` constrains or joins, and
an `@capture` names what a row prints.

The examples on this page run over two files:

````bash
mkdir -p docs
cat > docs/guide.md <<'EOF'
---
title: Guide
tags: [ops, release]
---

# Guide

Read the [runbook](runbook.md) and the [site](https://example.com/ops) first.

## Install

```sh
cargo install satz-cli
```

Run `make release` to ship.

## Usage

| Flag | Meaning |
| --- | --- |
| `--json` | rows as JSON |
| `--count` | row count |

- first item
- second item

## Usage

> Quoted advice.

1. one
2. two
EOF
cat > docs/notes.md <<'EOF'
# Notes

Inspect every changed function and report concrete failures with enough context to reproduce the problem.

## Review

Inspect every changed function and report each concrete failure with enough context to reproduce it.

:::note{kind=warning}
Directive body.
:::
EOF
````

A capture prints the node's address, the lines it spans, its type and its
first line:

```console
$ satz query '(heading :level 2) @h'
@h  docs/guide.md#install  L10  heading  Install
@h  docs/guide.md#usage  L18  heading  Usage
@h  docs/guide.md#usage-1  L28  heading  Usage
@h  docs/notes.md#review  L5  heading  Review
```

Here is how many headings each level has, most first:

```console
$ satz query '(heading :level ?l) @h (group ?l (count @h @n))' --sort '-@n'
?l=2
@h  4 nodes
@n  4

?l=1
@h  2 nodes
@n  2
```

## Examples

```
satz query '(section :title "Install" (code) @c)'
satz query '(section > (heading) @title (code :lang "sh") @c)'
satz query '(section :title "Install" (code) @c . (paragraph) @after)'
satz query '(link :url /^https:/ @u) @l'
satz query '[(code :lang "toml") (code :lang "yaml")] @c'
satz query '(list :ordered true (item)+ @items)'
satz query '(section :title "Usage" (table (row :header false (cell :column "Flag") @flag)))'
satz query '(entry :key "tags" :value ?tag @tag) @e'
satz query '(heading :norm ?t :node ?n) @first (heading :norm ?t :node (after ?n)) @repeat'
satz query '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (near ?t 0.3 @s)) @b'
satz query '(code :inline true :matches /^make (?<target>\w+)$/) @c (section :id ?target) @s'
satz query '(file :path ?p) @f (not (file :path ?p (table)))'
satz query '(file :path ?f (paragraph) @p) (group ?f (count @p @n) (sum :words @p @words))'
satz query '(paragraph :words ?w @w) @p' --sort '-@w' --limit 3
satz query '(section :level ?a > (section :level (> (+ ?a 1))) @skip)'
```

## Types

| Type | Keys |
|---|---|
| `file` | `:path`, the file's path as the command found it |
| `section` | `:id` (the heading's anchor, numbered when repeated: `usage`, `usage-1`), `:title`, `:level` |
| `heading` | `:level` |
| `paragraph`, `item`, `quote`, `table` | |
| `list` | `:ordered` |
| `code` | `:lang`; `:inline` (`true` for a code span, `false` for a block) and, on a span, `:ticks`, its backtick count |
| `row` | in a `table`: `:index` (`0` for the header) and `:header`; in lint data: `:table`, `:key` or `:index`, and `:value` |
| `cell` | `:index` and `:column`, the text of its column's header cell |
| `link`, `image` | `:url` |
| `frontmatter` | its scalar keys |
| `entry` | in `frontmatter`, one per scalar: `:key` (dotted for nested maps), `:value`, `:index` for a list item, and `:tag`, the YAML type, as in `!!str` |
| `directive` | `:name` and its attributes; inline, also `:label`, the text between its brackets or the label an inline rule claimed |
| `problem` | `:kind` (`frontmatter` or `markup`) and `:message` |
| `_` | any type, any key |

A section holds its heading and everything up to the next heading of the same
or a higher level. A table holds a row per line, the header first, and a row
holds a cell per column. A row's text is its line from pipe to pipe, and a
cell's text its trimmed content. Directives follow the generic directive
syntax: `:name[label]{attrs}` inline, `::name{attrs}` as a leaf and
`:::name{attrs}` around a block.
A program embedding satz can give the Markdown frontend further inline
syntax with `Markdown::with_inline_rule`: the text a rule claims reads as an
inline `directive`, with the name, label and attributes the rule gives it and
the position of the text it claimed. The `:name[label]` syntax goes first
where both match. The frontend's `Markdown::document` and
`Markdown::empty_document` make documents that render its rules;
`Document::parse` and `Document::new` use the default frontend, which has
none. See the crate documentation.

Markup that does not parse becomes a `problem` node under its file, such as an
unclosed directive or a repeated attribute. Frontmatter that is not valid YAML
leaves its file holding only the `problem` node.

Files other than Markdown hold paragraphs separated by blank lines.

Every type also takes these keys:

- `:text "exact"`, `:contains "case-insensitive"` and `:matches /regexp/`;
- `:words` and `:chars`, word and character counts, and `:lines`, the line
  count. A file counts its body, the text its frontend read without the
  frontmatter, though its own `:text` is empty;
- `:norm`, the text lower-cased with whitespace collapsed;
- `:node`, an id in document order;
- `:position`, the file path, then the offset;
- `:dynamic`, a flag a program embedding satz sets on text that rules about
  wording should skip. `satz query` sets it on nothing.

A program embedding satz can register further node types and keys, and a unit
type such as a document set, through its schema; see the crate documentation.

## Values

- `:level`, `:words`, `:chars` and `:lines` take a number or a range: `1..3`,
  `80..` or `..20`. A number may be decimal: `:level 2.0` is `:level 2`.
- Any numeric value takes `(> n)`, `(>= n)`, `(< n)` or `(<= n)`; a
  `YYYY-MM-DD` date takes `(older days)`, counted from now.
- A comparison also takes a `?variable` or arithmetic over variables,
  numbers and `$params`: `(+ a b …)`, `(- a b …)`, `(- a)`, `(* a b …)` and
  `(/ a b …)`; dividing by 0 matches nothing.
  Arithmetic alone compares equal: `:level (+ ?l 1)` is one level below `?l`,
  and `:words (> (* ?w 2))` more than twice `?w`. Values that are not numbers
  fail the comparison.
- Other values compare exactly. Quote them with `"`, or leave single words
  bare.
- A `/regexp/` matches any key, as in `:url /^https:/` or `:title /^Phase/`.
  `#"regexp"` is the same regexp quoted instead, so it holds `/` as is:
  `:url #"^https://example\.com/"`. Inside it, `\"` is a quote and every
  other backslash belongs to the regexp. See
  [Regular expressions](#regular-expressions).
- A named group binds the variable of its name:
  `:matches /^make (?<target>\w+)$/` binds `?target`. `:text` compares
  trimmed text, `:matches` the text as written.
- `(not value)` negates, as in `:lang (not "sh")` or `:id (not ?x)`.
- A `?variable` binds the key's value. Every other use of it must be equal,
  which joins the patterns that share it.
- A `$param` stands for a number or text the caller supplies. Lint rules take
  params from their module and from `satz.toml`; `satz query` supplies none.

Compared with a variable:

- `(after ?x)` and `(before ?x)` sort after or before it. `:node (after ?n)`
  lists each pair once and never pairs a node with itself.
- `(contains ?x)` holds it as a substring, ignoring case.
- `(covers ?x)` is it or a dotted prefix of it: `code.style` covers
  `code.style.go`.
- `(within ?x)` is it or a dotted extension of it: `code.style.go` lies within
  `code.style`.
- `(near ?t 0.6)` shares at least that Jaccard share of three-word shingles.
- `(overlap ?t 0.9)` shares at least that share of the shorter text's
  shingles.
- `(similar ?t 0.85)` has at least that cosine similarity under a local
  Model2Vec model: `--model DIR`, else `[lint.semantic] model_path` in
  `satz.toml`.

`near` and `overlap` intersect the shingle sets exactly. Two paragraphs that
say nearly the same thing:

```console
$ satz query '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (near ?t 0.3 @s)) @b'
@a  docs/notes.md#notes  L3  paragraph  Inspect every changed function and report concrete failures with enough context…
@s  0.3684210526315789
@b  docs/notes.md#review  L7  paragraph  Inspect every changed function and report each concrete failure with enough con…
$ satz query '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (overlap ?t 0.3 @s)) @b'
@a  docs/notes.md#notes  L3  paragraph  Inspect every changed function and report concrete failures with enough context…
@s  0.5384615384615384
@b  docs/notes.md#review  L7  paragraph  Inspect every changed function and report each concrete failure with enough con…
```

A threshold of 0 or below turns the comparison off: nothing matches it.
Model2Vec embeddings lose word order, so a negated sentence can score higher
than a paraphrase, and thresholds do not carry over between models or from
the lexical measures.

## Captures

A capture applies to what immediately precedes it, as in tree-sitter:

- After a pattern, it captures the node: `(code) @c`, `(link)? @l`.
- After a `?variable`, it captures the variable's value and prints it as a
  column: `(section :id ?s @s (code) @c)`. The capture counts as a use of the
  variable. `?` and `@` names never collide, so `?s @s` is fine.
- After any other value, it captures the node's value for that key:
  `:lang "sh" @l`, `:url /^https:/ @u`, `:words (> 10) @w`.
- After a similarity threshold or an aggregate, it captures the result:
  `(near ?t 0.6 @score)`, `(count @c @n)`.

A capture cannot follow nothing: `(section @s (code))` is an error. A value
capture takes a name of its own and holds one value per match, so it cannot
sit under `*` or `+` or inside `(not ...)`.

## Grammar

```
query    := form* (pattern | "(" "not" pattern ")")+ (group | edit+)?  ; patterns join on shared variables
edit     := "(" ("replace" | "insert-before" | "insert-after") @capture "template" ")"
          | "(" "delete" @capture ")"
form     := "(" "define" "(" name (?variable | @capture)* ")" pattern+ ")"
          | "(" "import" module ")"
pattern  := "(" type item* ")" quant? capture?  ; captures the node
          | "[" pattern+ "]" quant? capture?  ; alternatives
value    := "string" | word | number | range | /regexp/ | #"regexp" | ?variable
          | $param | "(" "not" value ")" | "(" comparison expr ")" | arith
          | "(" ("after" | "before" | "contains" | "covers" | "within") ?variable ")"
          | "(" ("near" | "overlap" | "similar") ?variable threshold capture? ")"
comparison := ">" | ">=" | "<" | "<=" | "older"  ; older takes a number or $param
expr     := number | $param | ?variable | arith
arith    := "(" ("+" | "-" | "*" | "/") expr+ ")"
item     := :key value capture? | pattern | ">" pattern | "."  ; captures the value
          | "(" "not" (pattern | relation) ")" | relation
relation := "(" ("reaches" | "inbound" | "to" | "from") pattern ")"
quant    := "?" | "*" | "+"  ; only "?" on a top-level pattern
capture  := "@" name
group    := "(" "group" key* aggregate* ")"
key      := ?variable | @capture
aggregate := "(" "count" (@capture | ?variable) capture? filter? ")"
          | "(" "sum" measure @capture capture? filter? ")"
          | "(" ("min" | "max") (measure @capture | :level @capture | ?variable | @value) capture? filter? ")"
measure  := ":words" | ":lines" | ":chars"
filter   := "(" (">" | ">=" | "<" | "<=") (number | $param) ")"
comment  := ";" to the end of the line
```

## Structure

- A nested pattern matches a descendant at any depth; `>` requires a direct
  child. A section's own heading is `> (heading)`, while `(heading)` also
  reaches the headings of its subsections.
- `.` between two nested patterns makes them adjacent siblings. Before the
  first or after the last, it pins the first or last child. Unanchored nested
  patterns match in document order.
- `?`, `*` and `+` make a sibling optional or repeated. An optional sibling
  falls back per enclosing row: when no candidate unifies with it, the row
  stays once with its captures empty.
- `[P1 P2]` matches either. Every alternative binds the same variables.
- `(not P)` rejects a node with a descendant matching P.
- The relations `(reaches P)`, `(inbound P)`, `(to P)` and `(from P)` follow
  links between units, the node type a program embedding satz registers in its
  schema together with the link targets. `satz query` registers no unit, so a
  query using a relation is an error there.

```console
$ satz query '(list . (item) @first)'
@first  docs/guide.md#usage  L25  item  first item
@first  docs/guide.md#usage-1  L32  item  one
$ satz query '(section :level 2 (quote)? @q) @s'
@q  -
@s  docs/guide.md#install  L10-16  section  Install

@q  -
@s  docs/guide.md#usage  L18-26  section  Usage

@q  docs/guide.md#usage-1  L30  quote  Quoted advice.
@s  docs/guide.md#usage-1  L28-33  section  Usage

@q  -
@s  docs/notes.md#review  L5-11  section  Review
```

## Joins

Patterns side by side match together, joined on the variables they share, and
print their captures.

- A top-level `P?` is a left join. The required patterns join first; then each
  row joins every match of `P`, or stays once with `P`'s captures empty. A
  query needs at least one required pattern.
- A top-level `(not P)` drops the rows for which P matches with the same
  variables. It must share a variable bound by a required pattern. Inside
  `(not P)`, a variable bound outside joins; one bound only inside stays
  local.
- A top-level `(not P)` connects the patterns whose variables it mentions
  outside an optional pattern or a nested `(not ...)`. They pair up, and the
  negation keeps the pairs P misses.
- A variable bound in an optional pattern may recur only after a required
  binder has matched, and there the optional mention compares. The required
  binder is an attribute of the pattern that holds the optional one, a
  required sibling written before it, or, for a top-level `P?`, any required
  pattern.

Files without a table:

```console
$ satz query '(file :path ?p) @f (not (file :path ?p (table)))'
@f  docs/notes.md  file
```

## Defines and imports

Forms before the patterns name patterns for the query to call, as lint
modules do (see [satz lint](lint.md#forms)):

- `(define (name ?var @capture ...) pattern...)` names a pattern. A call
  passes the variables and captures it binds; every other variable stays
  local to each call.
- `(import module)` brings in a module's defines: a std module as
  `std/structure`, or a module of the user's or the project's layer by its
  path.

```console
$ satz query '(define (shell @c) (code :lang "sh") @c) (section :level 2 (shell @c)) @s'
@c  docs/guide.md#install  L12-14  code  cargo install satz-cli
@s  docs/guide.md#install  L10-16  section  Install
```

A define that calls itself, directly or through other defines, or that is
defined more than once, is recursive. Each definition is a clause, and a
call matches what any clause derives: the clauses run over the tree and
over what they derived so far until nothing new follows. Every query
terminates, since values only ever come from the tree. A recursive define:

- takes `?variables` only, and is called at the top level of a query or a
  clause, joined to its patterns on shared variables, without a capture;
- may be negated at the top level of a query, as `(not (reach ?p ?p))`, but
  not inside the clauses of any recursive define.

Files that reach `c.md` through links, at any distance:

```
satz query '(define (reach ?a ?b) (file :path ?a (link :url ?b)))
            (define (reach ?a ?b) (file :path ?a (link :url ?m)) (reach ?m ?b))
            (file :path ?p) @f (reach ?p "c.md")'
```

## Grouping

`(group ...)` ends a query. It turns the rows into one row per distinct
combination of keys:

```console
$ satz query '(file :path ?f (paragraph) @p) (group ?f (count @p @n) (sum :words @p @words))'
?f=docs/guide.md
@p  7 nodes
@n  7
@words  26

?f=docs/notes.md
@p  3 nodes
@n  3
@words  32
```

- A `?variable` key groups by its value, as does a value capture. A node
  capture key groups by its nodes, so one node per `@f` gives one row per
  file. Rows where the key is unbound or empty form one group.
- Without keys, the whole result is one group, even when nothing matches.
  With keys, no match means no rows. Without aggregates, the rows are the
  distinct keys.
- `(count @c)` counts the distinct nodes of `@c` in the group, and
  `(count ?x)` or `(count @x)` the distinct values of a variable or value
  capture. Empty captures count 0, so `(file (code)? @c) @f` reports files
  without code as 0.
- `sum`, `min` and `max` read `:words`, `:lines` or `:chars` from the distinct
  nodes of a capture; `min` and `max` also read `:level`, or the numeric
  values of a `?variable` or value capture. With no operand, the result is
  unbound.
- A filter such as `(>= 3)` or `(< $min)` drops the groups that fail it, and
  those whose result is unbound.
- A capture after the aggregate names its result, as in `(count @c @n)`.
  Variables other than the keys are dropped; every capture stays, a value
  capture with the distinct values of its group.

```console
$ satz query '(file (code :inline false)? @c) @f (group @f (count @c @n))'
@c  1 node
@f  docs/guide.md  file
@n  1

@c  -
@f  docs/notes.md  file
@n  0
```

## Edits

Edit forms end a query instead of `(group ...)`. Each acts on every node a
capture holds, in every row:

- `(replace @c "template")` puts the template's text in place of the node.
- `(delete @c)` removes the node. A node on lines of its own goes with its
  line break, and with a blank line that would otherwise double the one
  before it or end the file.
- `(insert-before @c "template")` and `(insert-after @c "template")` insert
  the text where the node starts or ends. Line breaks are the template's
  own: write `\n` in the string.

A node spans its source from its first marker: a heading includes its `#`s,
a list item its bullet.

Templates take the placeholders of lint messages (see
[Rule keys](lint.md#rule-keys)), `{@c.text}` for a node's text among them,
and filters after `|`: `lower`, `upper`, `capitalize`, `trim` and
`times:TEXT`, which repeats `TEXT` as often as the value counts.

`satz query` prints the edits as a unified diff, and applies them with
`--write`, naming each file it wrote. Two edits that touch the same bytes
are an error, and nothing is written.

```console
$ satz query '(heading :level ?l :text ?t) @h (replace @h "{?l|times:#} {?t|upper}")' --path docs/notes.md
--- docs/notes.md
+++ docs/notes.md
@@ -1,1 +1,1 @@
-# Notes
+# NOTES
@@ -5,1 +5,1 @@
-## Review
+## REVIEW
```

## Rejected queries

The reader rejects a query with a message, the query line, a caret under the
offending token and, where it knows one, a suggested fix:

```console
$ satz query '(heading :text ?t) @h'
satz: ?t appears only once, so it matches anything
  (heading :text ?t) @h
                 ^
  use ?t again to join on it, or drop it
```

It rejects:

- a variable that appears once, since it would match anything;
- a variable that is compared but never bound, or bound only in an optional
  pattern and used again without an earlier required binder;
- top-level patterns that share no variable, directly or through a top-level
  `(not P)`, since every combination would match, and a top-level `(not P)`
  that shares none, since it would drop every row or none;
- alternatives that bind different variables, and captures inside
  `(not ...)`;
- a capture that follows no pattern or value, and two captures of one name
  where either is a value;
- `*` or `+` on a top-level pattern, a group key captured under `*` or `+`,
  and a value capture under `*` or `+`;
- a variable after an aggregate or a similarity threshold, which takes a
  capture, as in `(count @c @n)`;
- `(group ...)` inside a pattern, `sum` over a variable, and an aggregate
  `:key` other than those above;
- an unknown type, or a key its type does not take;
- a `--sort` name that no pattern binds or that `(group ...)` drops.

## Regular expressions

`:matches`, `/regexp/` and `#"regexp"` values compile with the Rust
[`regex`](https://docs.rs/regex) crate. Its syntax is RE2's, the syntax of
Go's `regexp`, with these differences:

- `\d`, `\w`, `\s` and `\b` are Unicode-aware: `\d` matches `٣` and `\w`
  matches `é`. Write `[0-9]` or `(?-u:\w)` for RE2's ASCII classes.
- Inside a class, `&&`, `--` and `~~` are intersection, difference and
  symmetric difference: `[a-z&&[^aeiou]]` is a consonant, where RE2 reads the
  class literally.
- `\<` and `\>` are start- and end-of-word boundaries; RE2 rejects them.
- `\Q…\E` quoting and `\C` are rejected; RE2 accepts them.

Named groups take either form, `(?<name>…)` or `(?P<name>…)`.

## Output

Each capture prints `@name  address  lines  type  first line`. The address is
the file's path, with the enclosing section's id after `#`. A file prints no
lines, and a first line longer than 80 characters is cut with `…`.

- `--text` prints each node's whole text after its line.
- `--json` prints rows keyed by capture; a node is an object of `address`,
  `type`, `attrs`, `lines` and `text`, and a value capture a string, or a
  number for scores and aggregates.
- `--count` prints the number of rows.
- `--limit N` keeps the first N rows and says on stderr how many it left out.

The output flags exclude each other.

A value capture prints as `@name  value`. A grouped row prints its variable
keys on one line, as `?l=2`, then its captures: each node of a key capture as
above, every other node capture as `@c  N nodes` (or `N matches` for an
unnamed pattern), and each value capture and aggregate as `@n  4`. `--text`
prints every node of every capture. `--json` adds each variable of a row
under a `"?name"` key. `--count` counts the groups.

`--sort KEY` orders the rows by a `?var`, a value capture `@n` or a key of a
node capture `@capture.key`, descending with a leading `-`: `--sort '-@n'`,
`--sort '?l'` or `--sort '-@h.level'`. Quote it, since shells expand `?`.

- A capture holding several nodes sorts by its first.
- Rows without a value sort last.
- Values compare as numbers when all of them are numbers, else as strings.
  Equal rows keep their order.
- Ungrouped rows print the sorted variable on their variable line.
- `--sort`, `--limit` and `--count` apply after grouping.

```console
$ satz query '(paragraph :words ?w) @p' --sort '-?w' --limit 2
?w=15
@p  docs/notes.md#notes  L3  paragraph  Inspect every changed function and report concrete failures with enough context…

?w=15
@p  docs/notes.md#review  L7  paragraph  Inspect every changed function and report each concrete failure with enough con…
satz: 2 of 10 rows shown
```

Rows go to stdout. A query that matches nothing prints `[]` under `--json`,
and otherwise `satz: no matches` on stderr; it still exits 0. A query the
reader rejects, a missing path or a missing model exits 2.

## Files

Without `--path`, `satz query` walks the current directory. `--path` names a
file or directory instead, and repeats.

- A directory yields every file whose extension a frontend claims: `.md` for
  Markdown. Below it, hidden entries, Git-ignored paths, `.git`,
  `node_modules` and `vendor` are skipped.
- A file named with `--path` is read whatever its extension; one no frontend
  claims reads as plain text.
- A file's path is its `--path` argument, cleaned, joined with the path below
  it.
- A file reached through several arguments is read once.
- A file that cannot be read is left out with a `satz: <path>: omitted: …`
  line on stderr, and the query runs over the rest. A missing path stops the
  query.
