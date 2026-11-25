# Similarity parity

Similarity compares `near` (Jaccard), `overlap` (containment) and `similar` (Model2Vec cosine) rows with their score captures (AD-22, AD-25). The model is the pinned directory `mise run model` fetches, passed as `$PARITY_MODEL`.

Both CLIs read only the config written into this file's working directory.

```scrut
$ export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/config" XDG_CACHE_HOME="$PWD/cache" XDG_DATA_HOME="$PWD/data" XDG_STATE_HOME="$PWD/state" HENIA_HARNESS=none
```

```scrut
$ cat > similar.md <<'EOF'
> # Similar
>
> The gate runs every formatter, linter and test before a commit lands on the main branch.
>
> The gate runs every formatter, linter and the tests before any commit lands on the main branch today.
>
> Before a change lands on main, the gate executes all formatters, linters and tests first.
>
> Bake the bread at two hundred degrees until the crust turns a deep golden brown.
> EOF
```

Jaccard over three-word shingles, compared literally:

```scrut
$ henia query --path similar.md '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (near ?t 0.1 @s)) @b' --json
[
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "s": 0.42857142857142855
  }
]
```

tela prints the same.

```scrut
$ tela query --path similar.md '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (near ?t 0.1 @s)) @b' --json
[
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "s": 0.42857142857142855
  }
]
```

Containment over the shorter paragraph's shingles, compared literally:

```scrut
$ henia query --path similar.md '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (overlap ?t 0.1 @s)) @b' --json
[
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "s": 0.6428571428571429
  }
]
```

tela prints the same.

```scrut
$ tela query --path similar.md '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (overlap ?t 0.1 @s)) @b' --json
[
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "s": 0.6428571428571429
  }
]
```

Model2Vec cosine; tela's scores must agree to six decimal places, so its lines match by regex:

```scrut
$ henia query --path similar.md --model "$PARITY_MODEL" '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (similar ?t 0.3 @s)) @b' --json
[
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "s": 0.9819888423909093
  },
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "Before a change lands on main, the gate executes all formatters, linters and tests first.",
      "type": "paragraph"
    },
    "s": 0.7430853956418999
  },
  {
    "?n": "000000005",
    "?t": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "Before a change lands on main, the gate executes all formatters, linters and tests first.",
      "type": "paragraph"
    },
    "s": 0.7978960898029744
  }
]
```

tela prints the same rows, scores within six decimals.

```scrut
$ tela query --path similar.md --model "$PARITY_MODEL" '(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (similar ?t 0.3 @s)) @b' --json
[
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "s": 0\.981988\d* (regex)
  },
  {
    "?n": "000000004",
    "?t": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "The gate runs every formatter, linter and test before a commit lands on the main branch.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "Before a change lands on main, the gate executes all formatters, linters and tests first.",
      "type": "paragraph"
    },
    "s": 0\.743085\d* (regex)
  },
  {
    "?n": "000000005",
    "?t": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
    "a": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.",
      "type": "paragraph"
    },
    "b": {
      "address": "similar.md#similar",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "Before a change lands on main, the gate executes all formatters, linters and tests first.",
      "type": "paragraph"
    },
    "s": 0\.797896\d* (regex)
  }
]
```
