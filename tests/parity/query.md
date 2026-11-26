# Query parity

Each pair runs the same pattern through the pinned henia and through satz over the same Markdown, and the rows must agree (AD-24, AD-25).

Both CLIs read only the config written into this file's working directory.

```scrut
$ export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/config" XDG_CACHE_HOME="$PWD/cache" XDG_DATA_HOME="$PWD/data" XDG_STATE_HOME="$PWD/state" HENIA_HARNESS=none
```

````scrut
$ mkdir -p docs && cat > docs/guide.md <<'EOF'
> ---
> title: Parity guide
> tags: [query, tree]
> version: 3
> ---
>
> # Guide
>
> Intro paragraph with a [link](other.md#part), an ![image](img.png) and `inline code`.
>
> ## Install
>
> ```bash
> mise install
> ```
>
> ```toml
> [tools]
> ```
>
> ## Tables
>
> | Name | Check |
> |------|-------|
> | alpha | `fd -e go` |
> | beta | |
>
> ## Install
>
> A second install section with a longer paragraph that spans
> two source lines.
> EOF
````

````scrut
$ mkdir -p docs && cat > docs/other.md <<'EOF'
> # Other
>
> ## Part
>
> ```bash
> cargo build
> ```
>
> - one
> - two
> EOF
````

Headings across two files, in path order:

```scrut
$ henia query --path docs '(heading) @h' --json
[
  {
    "h": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "level": "1"
      },
      "lines": [
        7,
        7
      ],
      "text": "Guide",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/guide.md#install",
      "attrs": {
        "level": "2"
      },
      "lines": [
        11,
        11
      ],
      "text": "Install",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "level": "2"
      },
      "lines": [
        21,
        21
      ],
      "text": "Tables",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "level": "2"
      },
      "lines": [
        28,
        28
      ],
      "text": "Install",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/other.md#other",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Other",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/other.md#part",
      "attrs": {
        "level": "2"
      },
      "lines": [
        3,
        3
      ],
      "text": "Part",
      "type": "heading"
    }
  }
]
```

satz prints the same.

```scrut
$ satz query --path docs '(heading) @h' --json
[
  {
    "h": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "level": "1"
      },
      "lines": [
        7,
        7
      ],
      "text": "Guide",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/guide.md#install",
      "attrs": {
        "level": "2"
      },
      "lines": [
        11,
        11
      ],
      "text": "Install",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "level": "2"
      },
      "lines": [
        21,
        21
      ],
      "text": "Tables",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "level": "2"
      },
      "lines": [
        28,
        28
      ],
      "text": "Install",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/other.md#other",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Other",
      "type": "heading"
    }
  },
  {
    "h": {
      "address": "docs/other.md#part",
      "attrs": {
        "level": "2"
      },
      "lines": [
        3,
        3
      ],
      "text": "Part",
      "type": "heading"
    }
  }
]
```

A section by id with the language of each code block bound and captured:

```scrut
$ henia query --path docs '(section :id "install" (code :lang ?l @lang) @c)' --json
[
  {
    "?l": "bash",
    "c": {
      "address": "docs/guide.md#install",
      "attrs": {
        "info": "bash",
        "inline": "false",
        "lang": "bash"
      },
      "lines": [
        13,
        15
      ],
      "text": "mise install\n",
      "type": "code"
    },
    "lang": "bash"
  },
  {
    "?l": "toml",
    "c": {
      "address": "docs/guide.md#install",
      "attrs": {
        "info": "toml",
        "inline": "false",
        "lang": "toml"
      },
      "lines": [
        17,
        19
      ],
      "text": "[tools]\n",
      "type": "code"
    },
    "lang": "toml"
  }
]
```

satz prints the same.

```scrut
$ satz query --path docs '(section :id "install" (code :lang ?l @lang) @c)' --json
[
  {
    "?l": "bash",
    "c": {
      "address": "docs/guide.md#install",
      "attrs": {
        "info": "bash",
        "inline": "false",
        "lang": "bash"
      },
      "lines": [
        13,
        15
      ],
      "text": "mise install\n",
      "type": "code"
    },
    "lang": "bash"
  },
  {
    "?l": "toml",
    "c": {
      "address": "docs/guide.md#install",
      "attrs": {
        "info": "toml",
        "inline": "false",
        "lang": "toml"
      },
      "lines": [
        17,
        19
      ],
      "text": "[tools]\n",
      "type": "code"
    },
    "lang": "toml"
  }
]
```

Code blocks grouped by language and counted:

```scrut
$ henia query --path docs '(code :inline false :lang ?l) @c (group ?l (count @c @n))' --json
[
  {
    "?l": "bash",
    "c": [
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          13,
          15
        ],
        "text": "mise install\n",
        "type": "code"
      },
      {
        "address": "docs/other.md#part",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          5,
          7
        ],
        "text": "cargo build\n",
        "type": "code"
      }
    ],
    "n": 2
  },
  {
    "?l": "toml",
    "c": {
      "address": "docs/guide.md#install",
      "attrs": {
        "info": "toml",
        "inline": "false",
        "lang": "toml"
      },
      "lines": [
        17,
        19
      ],
      "text": "[tools]\n",
      "type": "code"
    },
    "n": 1
  }
]
```

satz prints the same.

```scrut
$ satz query --path docs '(code :inline false :lang ?l) @c (group ?l (count @c @n))' --json
[
  {
    "?l": "bash",
    "c": [
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          13,
          15
        ],
        "text": "mise install\n",
        "type": "code"
      },
      {
        "address": "docs/other.md#part",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          5,
          7
        ],
        "text": "cargo build\n",
        "type": "code"
      }
    ],
    "n": 2
  },
  {
    "?l": "toml",
    "c": {
      "address": "docs/guide.md#install",
      "attrs": {
        "info": "toml",
        "inline": "false",
        "lang": "toml"
      },
      "lines": [
        17,
        19
      ],
      "text": "[tools]\n",
      "type": "code"
    },
    "n": 1
  }
]
```

Links and images; henia adds its link resolution attributes, satz keeps `url` only (AD-25):

```scrut
$ henia query --path docs '(link) @l' --json
[
  {
    "l": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "anchored": "true",
        "dest": "other.md#part",
        "exists": "true",
        "url": "other.md#part"
      },
      "lines": [
        9,
        9
      ],
      "text": "link",
      "type": "link"
    }
  }
]
```

satz prints the same rows without henia's link resolution attributes.

```scrut
$ satz query --path docs '(link) @l' --json
[
  {
    "l": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "url": "other.md#part"
      },
      "lines": [
        9,
        9
      ],
      "text": "link",
      "type": "link"
    }
  }
]
```

Table rows whose Check cell is empty:

```scrut
$ henia query --path docs '(table (row (cell :column "Check" :text "")) @r)' --json
[
  {
    "r": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "header": "false",
        "index": "2"
      },
      "lines": [
        26,
        26
      ],
      "text": "| beta | |",
      "type": "row"
    }
  }
]
```

satz prints the same.

```scrut
$ satz query --path docs '(table (row (cell :column "Check" :text "")) @r)' --json
[
  {
    "r": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "header": "false",
        "index": "2"
      },
      "lines": [
        26,
        26
      ],
      "text": "| beta | |",
      "type": "row"
    }
  }
]
```

Frontmatter entries with their YAML tags:

```scrut
$ henia query --path docs '(entry :key ?k @k) @e' --json
[
  {
    "?k": "title",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "key": "title",
        "tag": "!!str",
        "value": "Parity guide"
      },
      "lines": [
        2,
        2
      ],
      "text": "Parity guide",
      "type": "entry"
    },
    "k": "title"
  },
  {
    "?k": "tags",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "index": "0",
        "key": "tags",
        "tag": "!!str",
        "value": "query"
      },
      "lines": [
        3,
        3
      ],
      "text": "query",
      "type": "entry"
    },
    "k": "tags"
  },
  {
    "?k": "tags",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "index": "1",
        "key": "tags",
        "tag": "!!str",
        "value": "tree"
      },
      "lines": [
        3,
        3
      ],
      "text": "tree",
      "type": "entry"
    },
    "k": "tags"
  },
  {
    "?k": "version",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "key": "version",
        "tag": "!!int",
        "value": "3"
      },
      "lines": [
        4,
        4
      ],
      "text": "3",
      "type": "entry"
    },
    "k": "version"
  }
]
```

satz prints the same.

```scrut
$ satz query --path docs '(entry :key ?k @k) @e' --json
[
  {
    "?k": "title",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "key": "title",
        "tag": "!!str",
        "value": "Parity guide"
      },
      "lines": [
        2,
        2
      ],
      "text": "Parity guide",
      "type": "entry"
    },
    "k": "title"
  },
  {
    "?k": "tags",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "index": "0",
        "key": "tags",
        "tag": "!!str",
        "value": "query"
      },
      "lines": [
        3,
        3
      ],
      "text": "query",
      "type": "entry"
    },
    "k": "tags"
  },
  {
    "?k": "tags",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "index": "1",
        "key": "tags",
        "tag": "!!str",
        "value": "tree"
      },
      "lines": [
        3,
        3
      ],
      "text": "tree",
      "type": "entry"
    },
    "k": "tags"
  },
  {
    "?k": "version",
    "e": {
      "address": "docs/guide.md",
      "attrs": {
        "key": "version",
        "tag": "!!int",
        "value": "3"
      },
      "lines": [
        4,
        4
      ],
      "text": "3",
      "type": "entry"
    },
    "k": "version"
  }
]
```

Headings sorted by `:position`, last first:

```scrut
$ henia query --path docs '(heading :position ?p @pos) @h' --sort '-@pos' --json
[
  {
    "?p": "docs/other.md\u0000000000009",
    "h": {
      "address": "docs/other.md#part",
      "attrs": {
        "level": "2"
      },
      "lines": [
        3,
        3
      ],
      "text": "Part",
      "type": "heading"
    },
    "pos": "docs/other.md\u0000000000009"
  },
  {
    "?p": "docs/other.md\u0000000000000",
    "h": {
      "address": "docs/other.md#other",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Other",
      "type": "heading"
    },
    "pos": "docs/other.md\u0000000000000"
  },
  {
    "?p": "docs/guide.md\u0000000000235",
    "h": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "level": "2"
      },
      "lines": [
        28,
        28
      ],
      "text": "Install",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000235"
  },
  {
    "?p": "docs/guide.md\u0000000000155",
    "h": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "level": "2"
      },
      "lines": [
        21,
        21
      ],
      "text": "Tables",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000155"
  },
  {
    "?p": "docs/guide.md\u0000000000096",
    "h": {
      "address": "docs/guide.md#install",
      "attrs": {
        "level": "2"
      },
      "lines": [
        11,
        11
      ],
      "text": "Install",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000096"
  },
  {
    "?p": "docs/guide.md\u0000000000000",
    "h": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "level": "1"
      },
      "lines": [
        7,
        7
      ],
      "text": "Guide",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000000"
  }
]
```

satz prints the same.

```scrut
$ satz query --path docs '(heading :position ?p @pos) @h' --sort '-@pos' --json
[
  {
    "?p": "docs/other.md\u0000000000009",
    "h": {
      "address": "docs/other.md#part",
      "attrs": {
        "level": "2"
      },
      "lines": [
        3,
        3
      ],
      "text": "Part",
      "type": "heading"
    },
    "pos": "docs/other.md\u0000000000009"
  },
  {
    "?p": "docs/other.md\u0000000000000",
    "h": {
      "address": "docs/other.md#other",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Other",
      "type": "heading"
    },
    "pos": "docs/other.md\u0000000000000"
  },
  {
    "?p": "docs/guide.md\u0000000000235",
    "h": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "level": "2"
      },
      "lines": [
        28,
        28
      ],
      "text": "Install",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000235"
  },
  {
    "?p": "docs/guide.md\u0000000000155",
    "h": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "level": "2"
      },
      "lines": [
        21,
        21
      ],
      "text": "Tables",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000155"
  },
  {
    "?p": "docs/guide.md\u0000000000096",
    "h": {
      "address": "docs/guide.md#install",
      "attrs": {
        "level": "2"
      },
      "lines": [
        11,
        11
      ],
      "text": "Install",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000096"
  },
  {
    "?p": "docs/guide.md\u0000000000000",
    "h": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "level": "1"
      },
      "lines": [
        7,
        7
      ],
      "text": "Guide",
      "type": "heading"
    },
    "pos": "docs/guide.md\u0000000000000"
  }
]
```

Sections of at least three lines, with their line counts:

````scrut
$ henia query --path docs '(section :lines (>= 3) @l) @s' --json
[
  {
    "l": "25",
    "s": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "id": "guide",
        "level": "1",
        "title": "Guide"
      },
      "lines": [
        7,
        31
      ],
      "text": "# Guide\n\nIntro paragraph with a [link](other.md#part), an ![image](img.png) and `inline code`.\n\n## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```\n\n## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |\n\n## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  },
  {
    "l": "9",
    "s": {
      "address": "docs/guide.md#install",
      "attrs": {
        "id": "install",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        11,
        19
      ],
      "text": "## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```",
      "type": "section"
    }
  },
  {
    "l": "6",
    "s": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "id": "tables",
        "level": "2",
        "title": "Tables"
      },
      "lines": [
        21,
        26
      ],
      "text": "## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |",
      "type": "section"
    }
  },
  {
    "l": "4",
    "s": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "id": "install-1",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        28,
        31
      ],
      "text": "## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  },
  {
    "l": "10",
    "s": {
      "address": "docs/other.md#other",
      "attrs": {
        "id": "other",
        "level": "1",
        "title": "Other"
      },
      "lines": [
        1,
        10
      ],
      "text": "# Other\n\n## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  },
  {
    "l": "8",
    "s": {
      "address": "docs/other.md#part",
      "attrs": {
        "id": "part",
        "level": "2",
        "title": "Part"
      },
      "lines": [
        3,
        10
      ],
      "text": "## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  }
]
````

satz prints the same.

````scrut
$ satz query --path docs '(section :lines (>= 3) @l) @s' --json
[
  {
    "l": "25",
    "s": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "id": "guide",
        "level": "1",
        "title": "Guide"
      },
      "lines": [
        7,
        31
      ],
      "text": "# Guide\n\nIntro paragraph with a [link](other.md#part), an ![image](img.png) and `inline code`.\n\n## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```\n\n## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |\n\n## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  },
  {
    "l": "9",
    "s": {
      "address": "docs/guide.md#install",
      "attrs": {
        "id": "install",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        11,
        19
      ],
      "text": "## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```",
      "type": "section"
    }
  },
  {
    "l": "6",
    "s": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "id": "tables",
        "level": "2",
        "title": "Tables"
      },
      "lines": [
        21,
        26
      ],
      "text": "## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |",
      "type": "section"
    }
  },
  {
    "l": "4",
    "s": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "id": "install-1",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        28,
        31
      ],
      "text": "## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  },
  {
    "l": "10",
    "s": {
      "address": "docs/other.md#other",
      "attrs": {
        "id": "other",
        "level": "1",
        "title": "Other"
      },
      "lines": [
        1,
        10
      ],
      "text": "# Other\n\n## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  },
  {
    "l": "8",
    "s": {
      "address": "docs/other.md#part",
      "attrs": {
        "id": "part",
        "level": "2",
        "title": "Part"
      },
      "lines": [
        3,
        10
      ],
      "text": "## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  }
]
````

Sections with their code block count and summed code lines, most first:

````scrut
$ henia query --path docs '(section (code)? @c) @s (group @s (count @c @n) (sum :lines @c @l))' --sort '-@n' --json
[
  {
    "c": [
      {
        "address": "docs/guide.md#guide",
        "attrs": {
          "inline": "true",
          "ticks": "1"
        },
        "lines": [
          9,
          9
        ],
        "text": "inline code",
        "type": "code"
      },
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          13,
          15
        ],
        "text": "mise install\n",
        "type": "code"
      },
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "toml",
          "inline": "false",
          "lang": "toml"
        },
        "lines": [
          17,
          19
        ],
        "text": "[tools]\n",
        "type": "code"
      },
      {
        "address": "docs/guide.md#tables",
        "attrs": {
          "inline": "true",
          "ticks": "1"
        },
        "lines": [
          25,
          25
        ],
        "text": "fd -e go",
        "type": "code"
      }
    ],
    "l": 8,
    "n": 4,
    "s": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "id": "guide",
        "level": "1",
        "title": "Guide"
      },
      "lines": [
        7,
        31
      ],
      "text": "# Guide\n\nIntro paragraph with a [link](other.md#part), an ![image](img.png) and `inline code`.\n\n## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```\n\n## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |\n\n## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  },
  {
    "c": [
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          13,
          15
        ],
        "text": "mise install\n",
        "type": "code"
      },
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "toml",
          "inline": "false",
          "lang": "toml"
        },
        "lines": [
          17,
          19
        ],
        "text": "[tools]\n",
        "type": "code"
      }
    ],
    "l": 6,
    "n": 2,
    "s": {
      "address": "docs/guide.md#install",
      "attrs": {
        "id": "install",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        11,
        19
      ],
      "text": "## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```",
      "type": "section"
    }
  },
  {
    "c": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        25,
        25
      ],
      "text": "fd -e go",
      "type": "code"
    },
    "l": 1,
    "n": 1,
    "s": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "id": "tables",
        "level": "2",
        "title": "Tables"
      },
      "lines": [
        21,
        26
      ],
      "text": "## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |",
      "type": "section"
    }
  },
  {
    "c": {
      "address": "docs/other.md#part",
      "attrs": {
        "info": "bash",
        "inline": "false",
        "lang": "bash"
      },
      "lines": [
        5,
        7
      ],
      "text": "cargo build\n",
      "type": "code"
    },
    "l": 3,
    "n": 1,
    "s": {
      "address": "docs/other.md#other",
      "attrs": {
        "id": "other",
        "level": "1",
        "title": "Other"
      },
      "lines": [
        1,
        10
      ],
      "text": "# Other\n\n## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  },
  {
    "c": {
      "address": "docs/other.md#part",
      "attrs": {
        "info": "bash",
        "inline": "false",
        "lang": "bash"
      },
      "lines": [
        5,
        7
      ],
      "text": "cargo build\n",
      "type": "code"
    },
    "l": 3,
    "n": 1,
    "s": {
      "address": "docs/other.md#part",
      "attrs": {
        "id": "part",
        "level": "2",
        "title": "Part"
      },
      "lines": [
        3,
        10
      ],
      "text": "## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  },
  {
    "c": null,
    "l": null,
    "n": 0,
    "s": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "id": "install-1",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        28,
        31
      ],
      "text": "## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  }
]
````

satz prints the same.

````scrut
$ satz query --path docs '(section (code)? @c) @s (group @s (count @c @n) (sum :lines @c @l))' --sort '-@n' --json
[
  {
    "c": [
      {
        "address": "docs/guide.md#guide",
        "attrs": {
          "inline": "true",
          "ticks": "1"
        },
        "lines": [
          9,
          9
        ],
        "text": "inline code",
        "type": "code"
      },
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          13,
          15
        ],
        "text": "mise install\n",
        "type": "code"
      },
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "toml",
          "inline": "false",
          "lang": "toml"
        },
        "lines": [
          17,
          19
        ],
        "text": "[tools]\n",
        "type": "code"
      },
      {
        "address": "docs/guide.md#tables",
        "attrs": {
          "inline": "true",
          "ticks": "1"
        },
        "lines": [
          25,
          25
        ],
        "text": "fd -e go",
        "type": "code"
      }
    ],
    "l": 8,
    "n": 4,
    "s": {
      "address": "docs/guide.md#guide",
      "attrs": {
        "id": "guide",
        "level": "1",
        "title": "Guide"
      },
      "lines": [
        7,
        31
      ],
      "text": "# Guide\n\nIntro paragraph with a [link](other.md#part), an ![image](img.png) and `inline code`.\n\n## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```\n\n## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |\n\n## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  },
  {
    "c": [
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "bash",
          "inline": "false",
          "lang": "bash"
        },
        "lines": [
          13,
          15
        ],
        "text": "mise install\n",
        "type": "code"
      },
      {
        "address": "docs/guide.md#install",
        "attrs": {
          "info": "toml",
          "inline": "false",
          "lang": "toml"
        },
        "lines": [
          17,
          19
        ],
        "text": "[tools]\n",
        "type": "code"
      }
    ],
    "l": 6,
    "n": 2,
    "s": {
      "address": "docs/guide.md#install",
      "attrs": {
        "id": "install",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        11,
        19
      ],
      "text": "## Install\n\n```bash\nmise install\n```\n\n```toml\n[tools]\n```",
      "type": "section"
    }
  },
  {
    "c": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        25,
        25
      ],
      "text": "fd -e go",
      "type": "code"
    },
    "l": 1,
    "n": 1,
    "s": {
      "address": "docs/guide.md#tables",
      "attrs": {
        "id": "tables",
        "level": "2",
        "title": "Tables"
      },
      "lines": [
        21,
        26
      ],
      "text": "## Tables\n\n| Name | Check |\n|------|-------|\n| alpha | `fd -e go` |\n| beta | |",
      "type": "section"
    }
  },
  {
    "c": {
      "address": "docs/other.md#part",
      "attrs": {
        "info": "bash",
        "inline": "false",
        "lang": "bash"
      },
      "lines": [
        5,
        7
      ],
      "text": "cargo build\n",
      "type": "code"
    },
    "l": 3,
    "n": 1,
    "s": {
      "address": "docs/other.md#other",
      "attrs": {
        "id": "other",
        "level": "1",
        "title": "Other"
      },
      "lines": [
        1,
        10
      ],
      "text": "# Other\n\n## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  },
  {
    "c": {
      "address": "docs/other.md#part",
      "attrs": {
        "info": "bash",
        "inline": "false",
        "lang": "bash"
      },
      "lines": [
        5,
        7
      ],
      "text": "cargo build\n",
      "type": "code"
    },
    "l": 3,
    "n": 1,
    "s": {
      "address": "docs/other.md#part",
      "attrs": {
        "id": "part",
        "level": "2",
        "title": "Part"
      },
      "lines": [
        3,
        10
      ],
      "text": "## Part\n\n```bash\ncargo build\n```\n\n- one\n- two",
      "type": "section"
    }
  },
  {
    "c": null,
    "l": null,
    "n": 0,
    "s": {
      "address": "docs/guide.md#install-1",
      "attrs": {
        "id": "install-1",
        "level": "2",
        "title": "Install"
      },
      "lines": [
        28,
        31
      ],
      "text": "## Install\n\nA second install section with a longer paragraph that spans\ntwo source lines.",
      "type": "section"
    }
  }
]
````

The default text output: address, lines, type and first line per capture:

```scrut
$ henia query --path docs '(heading :level 2) @h'
@h  docs/guide.md#install  L11  heading  Install
@h  docs/guide.md#tables  L21  heading  Tables
@h  docs/guide.md#install-1  L28  heading  Install
@h  docs/other.md#part  L3  heading  Part
```

satz prints the same.

```scrut
$ satz query --path docs '(heading :level 2) @h'
@h  docs/guide.md#install  L11  heading  Install
@h  docs/guide.md#tables  L21  heading  Tables
@h  docs/guide.md#install-1  L28  heading  Install
@h  docs/other.md#part  L3  heading  Part
```

`--count` prints the number of rows:

```scrut
$ henia query --path docs '(paragraph) @p' --count
4
```

satz prints the same.

```scrut
$ satz query --path docs '(paragraph) @p' --count
4
```

`--limit` keeps the first rows:

```scrut
$ henia query --path docs '(paragraph) @p' --limit 1 --json
[
  {
    "p": {
      "address": "docs/guide.md#guide",
      "attrs": {},
      "lines": [
        9,
        9
      ],
      "text": "Intro paragraph with a [link](other.md#part), an ![image](img.png) and `inline code`.",
      "type": "paragraph"
    }
  }
]
```

satz prints the same.

```scrut
$ satz query --path docs '(paragraph) @p' --limit 1 --json
[
  {
    "p": {
      "address": "docs/guide.md#guide",
      "attrs": {},
      "lines": [
        9,
        9
      ],
      "text": "Intro paragraph with a [link](other.md#part), an ![image](img.png) and `inline code`.",
      "type": "paragraph"
    }
  }
]
```

File nodes; henia adds `file`, `kind` and `main`, satz's walker sets `path` only (FR-7, AD-25):

```scrut
$ henia query --path docs '(file) @f' --json
[
  {
    "f": {
      "address": "docs/guide.md",
      "attrs": {
        "file": "docs/guide.md",
        "kind": "document",
        "main": "false",
        "path": "docs/guide.md"
      },
      "type": "file"
    }
  },
  {
    "f": {
      "address": "docs/other.md",
      "attrs": {
        "file": "docs/other.md",
        "kind": "document",
        "main": "false",
        "path": "docs/other.md"
      },
      "type": "file"
    }
  }
]
```

satz prints the same rows with `path` as the only attribute.

```scrut
$ satz query --path docs '(file) @f' --json
[
  {
    "f": {
      "address": "docs/guide.md",
      "attrs": {
        "path": "docs/guide.md"
      },
      "type": "file"
    }
  },
  {
    "f": {
      "address": "docs/other.md",
      "attrs": {
        "path": "docs/other.md"
      },
      "type": "file"
    }
  }
]
```
