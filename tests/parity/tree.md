# Tree parity

Node trees compare as `(_) @n` rows in document order, since henia has no tree dump (AD-25).

Both CLIs read only the config written into this file's working directory.

```scrut
$ export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/config" XDG_CACHE_HOME="$PWD/cache" XDG_DATA_HOME="$PWD/data" XDG_STATE_HOME="$PWD/state" HENIA_HARNESS=none
```

```scrut
$ cat > tree.md <<'EOF'
> # Tree
>
> Paragraph with *emphasis*, **strong** and a [link](https://example.com).
>
> > A quote
> > over two lines.
>
> 1. first
> 2. second
>    - nested
>
> :::note{kind=warn}
> Inside the directive.
> :::
>
> | A | B |
> |---|---|
> | 1 | 2 |
>
> ---
>
> Final paragraph.
> EOF
```

Block and inline structure of one Markdown file:

```scrut
$ henia query --path tree.md '(_) @n' --json
[
  {
    "n": {
      "address": "tree.md",
      "attrs": {
        "file": "tree.md",
        "kind": "document",
        "main": "false",
        "path": "tree.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "id": "tree",
        "level": "1",
        "title": "Tree"
      },
      "lines": [
        1,
        22
      ],
      "text": "# Tree\n\nParagraph with *emphasis*, **strong** and a [link](https://example.com).\n\n\u003e A quote\n\u003e over two lines.\n\n1. first\n2. second\n   - nested\n\n:::note{kind=warn}\nInside the directive.\n:::\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n---\n\nFinal paragraph.",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Tree",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "Paragraph with *emphasis*, **strong** and a [link](https://example.com).",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "dest": "https://example.com",
        "url": "https://example.com"
      },
      "lines": [
        3,
        3
      ],
      "text": "link",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        5,
        6
      ],
      "text": "A quote\n\u003e over two lines.",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        5,
        6
      ],
      "text": "A quote\n\u003e over two lines.",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "ordered": "true"
      },
      "lines": [
        8,
        10
      ],
      "text": "first\n2. second\n   - nested",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        8,
        8
      ],
      "text": "first",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        8,
        8
      ],
      "text": "first",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        9,
        10
      ],
      "text": "second\n   - nested",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        9,
        9
      ],
      "text": "second",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        10,
        10
      ],
      "text": "nested",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        10,
        10
      ],
      "text": "nested",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        10,
        10
      ],
      "text": "nested",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "kind": "warn",
        "name": "note"
      },
      "lines": [
        12,
        14
      ],
      "text": ":::note{kind=warn}\nInside the directive.\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        13,
        13
      ],
      "text": "Inside the directive.",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        16,
        18
      ],
      "text": "| A | B |\n|---|---|\n| 1 | 2 |",
      "type": "table"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "header": "true",
        "index": "0"
      },
      "lines": [
        16,
        16
      ],
      "text": "| A | B |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "A",
        "index": "0"
      },
      "lines": [
        16,
        16
      ],
      "text": "A",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "B",
        "index": "1"
      },
      "lines": [
        16,
        16
      ],
      "text": "B",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "header": "false",
        "index": "1"
      },
      "lines": [
        18,
        18
      ],
      "text": "| 1 | 2 |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "A",
        "index": "0"
      },
      "lines": [
        18,
        18
      ],
      "text": "1",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "B",
        "index": "1"
      },
      "lines": [
        18,
        18
      ],
      "text": "2",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        20,
        20
      ],
      "text": "---",
      "type": "rule"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        22,
        22
      ],
      "text": "Final paragraph.",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only, and links carry `url` without henia's link resolution attributes (AD-25). tela writes `<`, `>` and `&` as is where Go escapes them as `\u003c`, `\u003e` and `\u0026` (AD-25).

```scrut
$ tela query --path tree.md '(_) @n' --json
[
  {
    "n": {
      "address": "tree.md",
      "attrs": {
        "path": "tree.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "id": "tree",
        "level": "1",
        "title": "Tree"
      },
      "lines": [
        1,
        22
      ],
      "text": "# Tree\n\nParagraph with *emphasis*, **strong** and a [link](https://example.com).\n\n> A quote\n> over two lines.\n\n1. first\n2. second\n   - nested\n\n:::note{kind=warn}\nInside the directive.\n:::\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n---\n\nFinal paragraph.",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Tree",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "Paragraph with *emphasis*, **strong** and a [link](https://example.com).",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "url": "https://example.com"
      },
      "lines": [
        3,
        3
      ],
      "text": "link",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        5,
        6
      ],
      "text": "A quote\n> over two lines.",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        5,
        6
      ],
      "text": "A quote\n> over two lines.",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "ordered": "true"
      },
      "lines": [
        8,
        10
      ],
      "text": "first\n2. second\n   - nested",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        8,
        8
      ],
      "text": "first",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        8,
        8
      ],
      "text": "first",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        9,
        10
      ],
      "text": "second\n   - nested",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        9,
        9
      ],
      "text": "second",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        10,
        10
      ],
      "text": "nested",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        10,
        10
      ],
      "text": "nested",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        10,
        10
      ],
      "text": "nested",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "kind": "warn",
        "name": "note"
      },
      "lines": [
        12,
        14
      ],
      "text": ":::note{kind=warn}\nInside the directive.\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        13,
        13
      ],
      "text": "Inside the directive.",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        16,
        18
      ],
      "text": "| A | B |\n|---|---|\n| 1 | 2 |",
      "type": "table"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "header": "true",
        "index": "0"
      },
      "lines": [
        16,
        16
      ],
      "text": "| A | B |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "A",
        "index": "0"
      },
      "lines": [
        16,
        16
      ],
      "text": "A",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "B",
        "index": "1"
      },
      "lines": [
        16,
        16
      ],
      "text": "B",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "header": "false",
        "index": "1"
      },
      "lines": [
        18,
        18
      ],
      "text": "| 1 | 2 |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "A",
        "index": "0"
      },
      "lines": [
        18,
        18
      ],
      "text": "1",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {
        "column": "B",
        "index": "1"
      },
      "lines": [
        18,
        18
      ],
      "text": "2",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        20,
        20
      ],
      "text": "---",
      "type": "rule"
    }
  },
  {
    "n": {
      "address": "tree.md#tree",
      "attrs": {},
      "lines": [
        22,
        22
      ],
      "text": "Final paragraph.",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > front.md <<'EOF'
> ---
> name: tags
> count: 12
> ratio: 0.5
> enabled: true
> empty: null
> quoted: "123"
> date: 2026-11-25
> list: [a, b]
> nested:
>   key: value
> henia.lint.disable: duplicate-heading
> ---
>
> # Body
> EOF
```

Frontmatter becomes a `frontmatter` head with an `entry` per key, each with its YAML tag (FR-8):

```scrut
$ henia query --path front.md '(_) @n' --json
[
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "file": "front.md",
        "kind": "document",
        "main": "false",
        "path": "front.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "count": "12",
        "enabled": "true",
        "henia.lint.disable": "duplicate-heading",
        "name": "tags",
        "quoted": "123",
        "ratio": "0.5"
      },
      "lines": [
        1,
        14
      ],
      "text": "name: tags\ncount: 12\nratio: 0.5\nenabled: true\nempty: null\nquoted: \"123\"\ndate: 2026-11-25\nlist: [a, b]\nnested:\n  key: value\nhenia.lint.disable: duplicate-heading",
      "type": "frontmatter"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "name",
        "tag": "!!str",
        "value": "tags"
      },
      "lines": [
        2,
        2
      ],
      "text": "tags",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "count",
        "tag": "!!int",
        "value": "12"
      },
      "lines": [
        3,
        3
      ],
      "text": "12",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "ratio",
        "tag": "!!float",
        "value": "0.5"
      },
      "lines": [
        4,
        4
      ],
      "text": "0.5",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "enabled",
        "tag": "!!bool",
        "value": "true"
      },
      "lines": [
        5,
        5
      ],
      "text": "true",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "empty",
        "tag": "!!null",
        "value": "null"
      },
      "lines": [
        6,
        6
      ],
      "text": "null",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "quoted",
        "tag": "!!str",
        "value": "123"
      },
      "lines": [
        7,
        7
      ],
      "text": "123",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "date",
        "tag": "!!timestamp",
        "value": "2026-11-25"
      },
      "lines": [
        8,
        8
      ],
      "text": "2026-11-25",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "index": "0",
        "key": "list",
        "tag": "!!str",
        "value": "a"
      },
      "lines": [
        9,
        9
      ],
      "text": "a",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "index": "1",
        "key": "list",
        "tag": "!!str",
        "value": "b"
      },
      "lines": [
        9,
        9
      ],
      "text": "b",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "nested.key",
        "tag": "!!str",
        "value": "value"
      },
      "lines": [
        11,
        11
      ],
      "text": "value",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "henia.lint.disable",
        "tag": "!!str",
        "value": "duplicate-heading"
      },
      "lines": [
        12,
        12
      ],
      "text": "duplicate-heading",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md#body",
      "attrs": {
        "id": "body",
        "level": "1",
        "title": "Body"
      },
      "lines": [
        15,
        15
      ],
      "text": "# Body",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "front.md#body",
      "attrs": {
        "level": "1"
      },
      "lines": [
        15,
        15
      ],
      "text": "Body",
      "type": "heading"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path front.md '(_) @n' --json
[
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "path": "front.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "count": "12",
        "enabled": "true",
        "henia.lint.disable": "duplicate-heading",
        "name": "tags",
        "quoted": "123",
        "ratio": "0.5"
      },
      "lines": [
        1,
        14
      ],
      "text": "name: tags\ncount: 12\nratio: 0.5\nenabled: true\nempty: null\nquoted: \"123\"\ndate: 2026-11-25\nlist: [a, b]\nnested:\n  key: value\nhenia.lint.disable: duplicate-heading",
      "type": "frontmatter"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "name",
        "tag": "!!str",
        "value": "tags"
      },
      "lines": [
        2,
        2
      ],
      "text": "tags",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "count",
        "tag": "!!int",
        "value": "12"
      },
      "lines": [
        3,
        3
      ],
      "text": "12",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "ratio",
        "tag": "!!float",
        "value": "0.5"
      },
      "lines": [
        4,
        4
      ],
      "text": "0.5",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "enabled",
        "tag": "!!bool",
        "value": "true"
      },
      "lines": [
        5,
        5
      ],
      "text": "true",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "empty",
        "tag": "!!null",
        "value": "null"
      },
      "lines": [
        6,
        6
      ],
      "text": "null",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "quoted",
        "tag": "!!str",
        "value": "123"
      },
      "lines": [
        7,
        7
      ],
      "text": "123",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "date",
        "tag": "!!timestamp",
        "value": "2026-11-25"
      },
      "lines": [
        8,
        8
      ],
      "text": "2026-11-25",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "index": "0",
        "key": "list",
        "tag": "!!str",
        "value": "a"
      },
      "lines": [
        9,
        9
      ],
      "text": "a",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "index": "1",
        "key": "list",
        "tag": "!!str",
        "value": "b"
      },
      "lines": [
        9,
        9
      ],
      "text": "b",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "nested.key",
        "tag": "!!str",
        "value": "value"
      },
      "lines": [
        11,
        11
      ],
      "text": "value",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md",
      "attrs": {
        "key": "henia.lint.disable",
        "tag": "!!str",
        "value": "duplicate-heading"
      },
      "lines": [
        12,
        12
      ],
      "text": "duplicate-heading",
      "type": "entry"
    }
  },
  {
    "n": {
      "address": "front.md#body",
      "attrs": {
        "id": "body",
        "level": "1",
        "title": "Body"
      },
      "lines": [
        15,
        15
      ],
      "text": "# Body",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "front.md#body",
      "attrs": {
        "level": "1"
      },
      "lines": [
        15,
        15
      ],
      "text": "Body",
      "type": "heading"
    }
  }
]
```

```scrut
$ cat > errors.md <<'EOF'
> # Errors
>
> :::note{kind=
> broken attribute
> :::
>
> ::::outer
> :::inner
> unclosed inner
> ::::
>
> Text with :leaf[label]{=bad} inline.
> EOF
```

Malformed directives produce `problem` rows (FR-9):

```scrut
$ henia query --path errors.md '(_) @n' --json
[
  {
    "n": {
      "address": "errors.md",
      "attrs": {
        "file": "errors.md",
        "kind": "document",
        "main": "false",
        "path": "errors.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "errors.md",
      "attrs": {
        "kind": "markup",
        "message": "3:14: attribute \"kind\": missing value\n7:1: unclosed-directive: directive outer is not closed\n12:24: invalid attribute name"
      },
      "lines": [
        3,
        3
      ],
      "text": "3:14: attribute \"kind\": missing value\n7:1: unclosed-directive: directive outer is not closed\n12:24: invalid attribute name",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "id": "errors",
        "level": "1",
        "title": "Errors"
      },
      "lines": [
        1,
        12
      ],
      "text": "# Errors\n\n:::note{kind=\nbroken attribute\n:::\n\n::::outer\n:::inner\nunclosed inner\n::::\n\nText with :leaf[label]{=bad} inline.",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Errors",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "name": "note"
      },
      "lines": [
        3,
        5
      ],
      "text": ":::note{kind=\nbroken attribute\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "broken attribute",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "name": "outer"
      },
      "lines": [
        7,
        12
      ],
      "text": "::::outer\n:::inner\nunclosed inner\n::::\n\nText with :leaf[label]{=bad} inline.",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "name": "inner"
      },
      "lines": [
        8,
        10
      ],
      "text": ":::inner\nunclosed inner\n::::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {},
      "lines": [
        9,
        9
      ],
      "text": "unclosed inner",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {},
      "lines": [
        12,
        12
      ],
      "text": "Text with :leaf[label]{=bad} inline.",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path errors.md '(_) @n' --json
[
  {
    "n": {
      "address": "errors.md",
      "attrs": {
        "path": "errors.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "errors.md",
      "attrs": {
        "kind": "markup",
        "message": "3:14: attribute \"kind\": missing value\n7:1: unclosed-directive: directive outer is not closed\n12:24: invalid attribute name"
      },
      "lines": [
        3,
        3
      ],
      "text": "3:14: attribute \"kind\": missing value\n7:1: unclosed-directive: directive outer is not closed\n12:24: invalid attribute name",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "id": "errors",
        "level": "1",
        "title": "Errors"
      },
      "lines": [
        1,
        12
      ],
      "text": "# Errors\n\n:::note{kind=\nbroken attribute\n:::\n\n::::outer\n:::inner\nunclosed inner\n::::\n\nText with :leaf[label]{=bad} inline.",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Errors",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "name": "note"
      },
      "lines": [
        3,
        5
      ],
      "text": ":::note{kind=\nbroken attribute\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "broken attribute",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "name": "outer"
      },
      "lines": [
        7,
        12
      ],
      "text": "::::outer\n:::inner\nunclosed inner\n::::\n\nText with :leaf[label]{=bad} inline.",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {
        "name": "inner"
      },
      "lines": [
        8,
        10
      ],
      "text": ":::inner\nunclosed inner\n::::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {},
      "lines": [
        9,
        9
      ],
      "text": "unclosed inner",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "errors.md#errors",
      "attrs": {},
      "lines": [
        12,
        12
      ],
      "text": "Text with :leaf[label]{=bad} inline.",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > notes.txt <<'EOF'
> First plain paragraph
> over two lines.
>
> Second plain paragraph with # no heading and [no](link).
> EOF
```

A non-Markdown file named on the command line is read as plain paragraphs (FR-7, FR-11):

```scrut
$ henia query --path notes.txt '(_) @n' --json
[
  {
    "n": {
      "address": "notes.txt",
      "attrs": {
        "file": "notes.txt",
        "kind": "document",
        "main": "false",
        "path": "notes.txt"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "notes.txt",
      "attrs": {},
      "lines": [
        1,
        2
      ],
      "text": "First plain paragraph\nover two lines.",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "notes.txt",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "Second plain paragraph with # no heading and [no](link).",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path notes.txt '(_) @n' --json
[
  {
    "n": {
      "address": "notes.txt",
      "attrs": {
        "path": "notes.txt"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "notes.txt",
      "attrs": {},
      "lines": [
        1,
        2
      ],
      "text": "First plain paragraph\nover two lines.",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "notes.txt",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "Second plain paragraph with # no heading and [no](link).",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > links.md <<'EOF'
> # Links
>
> [ref]: https://example.com/a
>
> See <https://example.com/auto> and [x](a\_b.md) and [y](a&amp;b.md).
> EOF
```

henia keeps three goldmark quirks that tela fixes (AD-20, AD-27): the definition-only paragraph becomes an empty paragraph on line 1, the autolink loses its line and text, and destinations stay escaped.

```scrut
$ henia query --path links.md '(_) @n' --json
[
  {
    "n": {
      "address": "links.md",
      "attrs": {
        "file": "links.md",
        "kind": "document",
        "main": "false",
        "path": "links.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "id": "links",
        "level": "1",
        "title": "Links"
      },
      "lines": [
        1,
        5
      ],
      "text": "# Links\n\n[ref]: https://example.com/a\n\nSee \u003chttps://example.com/auto\u003e and [x](a\\_b.md) and [y](a\u0026amp;b.md).",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Links",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "See \u003chttps://example.com/auto\u003e and [x](a\\_b.md) and [y](a\u0026amp;b.md).",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "dest": "https://example.com/auto",
        "url": "https://example.com/auto"
      },
      "lines": [
        1,
        1
      ],
      "text": "",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "dest": "a\\_b.md",
        "exists": "false",
        "url": "a\\_b.md"
      },
      "lines": [
        5,
        5
      ],
      "text": "x",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "dest": "a\u0026amp;b.md",
        "exists": "false",
        "url": "a\u0026amp;b.md"
      },
      "lines": [
        5,
        5
      ],
      "text": "y",
      "type": "link"
    }
  }
]
```

tela drops the empty paragraph, places the autolink on line 5 with its URL as text, and unescapes the destinations. Its links carry `url` without henia's link resolution attributes, and it writes `<`, `>` and `&` as is where Go escapes them as `\u003c`, `\u003e` and `\u0026` (AD-25).

```scrut
$ tela query --path links.md '(_) @n' --json
[
  {
    "n": {
      "address": "links.md",
      "attrs": {
        "path": "links.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "id": "links",
        "level": "1",
        "title": "Links"
      },
      "lines": [
        1,
        5
      ],
      "text": "# Links\n\n[ref]: https://example.com/a\n\nSee <https://example.com/auto> and [x](a\\_b.md) and [y](a&amp;b.md).",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Links",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "See <https://example.com/auto> and [x](a\\_b.md) and [y](a&amp;b.md).",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "url": "https://example.com/auto"
      },
      "lines": [
        5,
        5
      ],
      "text": "https://example.com/auto",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "url": "a_b.md"
      },
      "lines": [
        5,
        5
      ],
      "text": "x",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "links.md#links",
      "attrs": {
        "url": "a&b.md"
      },
      "lines": [
        5,
        5
      ],
      "text": "y",
      "type": "link"
    }
  }
]
```
