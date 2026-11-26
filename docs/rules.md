# Std rules

tela embeds four lint modules. Their rules run by default under
[`tela lint`](lint.md); `[lint.config.<rule>]` in `tela.toml` sets their
params and data, and `disable` turns them off.

| Module | Rules | Params and data |
|---|---|---|
| `std/structure` | `duplicate-heading`; define `repeated-heading` | |
| `std/duplicates` | `duplicate-content` | `min-words` (12) |
| `std/similarity` | `similar-content`, `semantic-content`; define `paragraph-at` | `min-words` (12), `similarity` (0), `containment` (0), `threshold` (0) |
| `std/links` | `outdated-reference` | `outdated` table, empty |

Every std rule reports a warning. The modules live in the crate's `std/`
directory as ordinary [modules](lint.md#modules), each rule with its Matches
and Passes examples, which `tela lint test` runs.

## duplicate-heading

A heading repeated in one document makes its anchors ambiguous. The rule
reports each repeat at its heading, with the first heading of the same text
in the same file as the related location. Headings compare by `:norm`, so
case and spacing do not matter; empty headings are left out.

```bash
mkdir -p docs
cat > docs/setup.md <<'EOF'
# Setup

## Install

## install
EOF
```

```console
$ tela lint docs/setup.md
docs/setup.md:5:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
```

`(import std/structure)` brings in its define,
`(repeated-heading @first @repeat ?text)`, which matches each repeat with
the first heading of its text.

## duplicate-content

A paragraph of at least `min-words` words copied verbatim elsewhere in the
files linted together, in the same file or another. The rule reports each
later copy at its paragraph, with the first occurrence as the related
location. Paragraphs compare by `:norm`, and the text is the Markdown as
written, so two links with equal labels and different destinations differ.
Text marked `:dynamic` is left out.

```bash
cat > docs/a.md <<'EOF'
Run the formatter, the linter and the tests before every commit to the main branch.
EOF
cat > docs/b.md <<'EOF'
Run the formatter, the linter and the tests before every commit to the main branch.
EOF
```

```console
$ tela lint docs/a.md docs/b.md
docs/b.md:1:1: warning [duplicate-content] paragraph duplicates docs/a.md:1
```

## similar-content

A paragraph that repeats an earlier one in other words, by word shingles.
The rule compares each paragraph of at least `min-words` words with the
earliest earlier paragraph that reaches a threshold and reports it there:

- `similarity` is the Jaccard share of three-word shingles the two
  paragraphs have in common, as `(near …)` in [tq](query.md#values);
- `containment` is the share of the shorter paragraph's shingles the other
  holds, as `(overlap …)`, which catches a paragraph copied into a longer
  one.

Both are 0, and the rule silent, until set. Exact copies are left to
`duplicate-content`, and a paragraph found by Jaccard is not reported again
for containment. A finding carries the score, its method and up to five
shared phrases.

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

## semantic-content

A paragraph that may say what an earlier one says, by cosine similarity
under a local Model2Vec model. It compares each paragraph of at least
`min-words` words with the earliest earlier paragraph whose cosine reaches
`threshold`. It runs only with `[lint.semantic]` enabled and a
`model_path`, and stays silent while `threshold` is 0:

```toml
[lint.semantic]
enabled = true
model_path = "/path/to/potion-base-2M"

[lint.config.semantic-content]
threshold = 0.5
similarity = 0.3
```

Its own `similarity` and `containment` params leave out the paragraphs that
are lexically similar to an earlier one; set them to the values
`similar-content` uses, so a pair is reported by one rule. A finding carries
the cosine score and the model, and its message asks for a review of meaning
and constraints: embeddings lose word order, so a negated sentence can score
higher than a paraphrase.

## outdated-reference

Names listed in the `outdated` table, each mapped to its replacement. The
rule reports every paragraph, heading and table whose text contains an
outdated name, ignoring case, located at the name's first occurrence as
listed, else at the node. The
text is the Markdown as written, so code spans and link destinations
count; code blocks do not.

```bash
cat > docs/models.md <<'EOF'
# Models

Use `legacy-model` for drafts; see [the notes](legacy-model.md).
EOF
cat > tela.toml <<'EOF'
[lint.config.outdated-reference.outdated]
legacy-model = "current-model"
EOF
```

```console
$ tela lint docs/models.md
docs/models.md:3:6: warning [outdated-reference] "legacy-model" is marked outdated; use "current-model"
```
