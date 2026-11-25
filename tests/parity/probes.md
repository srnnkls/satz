# Adversarial probes

The spike's adversarial probes (`reference/spike/adv/`), each compared as `(_) @n` rows (AD-26). The spike matched 42 of 46; the rest are port fixes (TLA-012).

Both CLIs read only the config written into this file's working directory.

```scrut
$ export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/config" XDG_CACHE_HOME="$PWD/cache" XDG_DATA_HOME="$PWD/data" XDG_STATE_HOME="$PWD/state" HENIA_HARNESS=none
```

````scrut
$ cat > a01.md <<'EOF'
> :::note
> ```
> :::
> ```
> :::
> EOF
````

Probe `a01`:

````scrut
$ henia query --path a01.md '(_) @n' --json
[
  {
    "n": {
      "address": "a01.md",
      "attrs": {
        "file": "a01.md",
        "kind": "document",
        "main": "false",
        "path": "a01.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a01.md",
      "attrs": {
        "name": "note"
      },
      "lines": [
        1,
        5
      ],
      "text": ":::note\n```\n:::\n```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a01.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        2,
        4
      ],
      "text": ":::\n",
      "type": "code"
    }
  }
]
````

tela prints the same rows; the file node carries `path` only.

````scrut
$ tela query --path a01.md '(_) @n' --json
[
  {
    "n": {
      "address": "a01.md",
      "attrs": {
        "path": "a01.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a01.md",
      "attrs": {
        "name": "note"
      },
      "lines": [
        1,
        5
      ],
      "text": ":::note\n```\n:::\n```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a01.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        2,
        4
      ],
      "text": ":::\n",
      "type": "code"
    }
  }
]
````

````scrut
$ cat > a02.md <<'EOF'
> :::outer
> - item
>
>   ```
>   :::
>   ```
> :::
> EOF
````

Probe `a02`:

````scrut
$ henia query --path a02.md '(_) @n' --json
[
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "file": "a02.md",
        "kind": "document",
        "main": "false",
        "path": "a02.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "name": "outer"
      },
      "lines": [
        1,
        7
      ],
      "text": ":::outer\n- item\n\n  ```\n  :::\n  ```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        2,
        6
      ],
      "text": "item\n\n  ```\n  :::\n  ```",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {},
      "lines": [
        2,
        6
      ],
      "text": "item\n\n  ```\n  :::\n  ```",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "item",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        4,
        6
      ],
      "text": ":::\n",
      "type": "code"
    }
  }
]
````

tela prints the same rows; the file node carries `path` only.

````scrut
$ tela query --path a02.md '(_) @n' --json
[
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "path": "a02.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "name": "outer"
      },
      "lines": [
        1,
        7
      ],
      "text": ":::outer\n- item\n\n  ```\n  :::\n  ```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        2,
        6
      ],
      "text": "item\n\n  ```\n  :::\n  ```",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {},
      "lines": [
        2,
        6
      ],
      "text": "item\n\n  ```\n  :::\n  ```",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "item",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a02.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        4,
        6
      ],
      "text": ":::\n",
      "type": "code"
    }
  }
]
````

````scrut
$ cat > a03.md <<'EOF'
> :::outer
> > ```
> :::
> EOF
````

Probe `a03`:

````scrut
$ henia query --path a03.md '(_) @n' --json
[
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "file": "a03.md",
        "kind": "document",
        "main": "false",
        "path": "a03.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive outer is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive outer is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "name": "outer"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::outer\n\u003e ```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {},
      "lines": [
        2,
        3
      ],
      "text": "\u003e ```\n:::",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        2,
        3
      ],
      "text": "\u003e ```\n:::",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": ":::",
      "type": "paragraph"
    }
  }
]
````

tela prints the same rows; the file node carries `path` only.

````scrut
$ tela query --path a03.md '(_) @n' --json
[
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "path": "a03.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive outer is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive outer is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "name": "outer"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::outer\n> ```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {},
      "lines": [
        2,
        3
      ],
      "text": "> ```\n:::",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        2,
        3
      ],
      "text": "> ```\n:::",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a03.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": ":::",
      "type": "paragraph"
    }
  }
]
````

```scrut
$ cat > a04.md <<'EOF'
> :::a
> ::::b
> inner
> :::
> after
> ::::
> EOF
```

Probe `a04`:

```scrut
$ henia query --path a04.md '(_) @n' --json
[
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "file": "a04.md",
        "kind": "document",
        "main": "false",
        "path": "a04.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive a is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive a is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "name": "a"
      },
      "lines": [
        1,
        6
      ],
      "text": ":::a\n::::b\ninner\n:::\nafter\n::::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "name": "b"
      },
      "lines": [
        2,
        6
      ],
      "text": "::::b\ninner\n:::\nafter\n::::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {},
      "lines": [
        3,
        5
      ],
      "text": "inner\n:::\nafter",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a04.md '(_) @n' --json
[
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "path": "a04.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive a is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive a is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "name": "a"
      },
      "lines": [
        1,
        6
      ],
      "text": ":::a\n::::b\ninner\n:::\nafter\n::::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {
        "name": "b"
      },
      "lines": [
        2,
        6
      ],
      "text": "::::b\ninner\n:::\nafter\n::::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a04.md",
      "attrs": {},
      "lines": [
        3,
        5
      ],
      "text": "inner\n:::\nafter",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a05.md <<'EOF'
> :::a
> <div>
> :::
> </div>
>
> :::
> EOF
```

Probe `a05`:

```scrut
$ henia query --path a05.md '(_) @n' --json
[
  {
    "n": {
      "address": "a05.md",
      "attrs": {
        "file": "a05.md",
        "kind": "document",
        "main": "false",
        "path": "a05.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a05.md",
      "attrs": {
        "name": "a"
      },
      "lines": [
        1,
        6
      ],
      "text": ":::a\n\u003cdiv\u003e\n:::\n\u003c/div\u003e\n\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a05.md",
      "attrs": {},
      "lines": [
        2,
        4
      ],
      "text": "\u003cdiv\u003e\n:::\n\u003c/div\u003e",
      "type": "html"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a05.md '(_) @n' --json
[
  {
    "n": {
      "address": "a05.md",
      "attrs": {
        "path": "a05.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a05.md",
      "attrs": {
        "name": "a"
      },
      "lines": [
        1,
        6
      ],
      "text": ":::a\n<div>\n:::\n</div>\n\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a05.md",
      "attrs": {},
      "lines": [
        2,
        4
      ],
      "text": "<div>\n:::\n</div>",
      "type": "html"
    }
  }
]
```

```scrut
$ cat > a06.md <<'EOF'
> :::a
> <!--
> :::
> -->
> :::
> EOF
```

Probe `a06`:

```scrut
$ henia query --path a06.md '(_) @n' --json
[
  {
    "n": {
      "address": "a06.md",
      "attrs": {
        "file": "a06.md",
        "kind": "document",
        "main": "false",
        "path": "a06.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a06.md",
      "attrs": {
        "name": "a"
      },
      "lines": [
        1,
        5
      ],
      "text": ":::a\n\u003c!--\n:::\n--\u003e\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a06.md",
      "attrs": {},
      "lines": [
        2,
        4
      ],
      "text": "\u003c!--\n:::\n--\u003e",
      "type": "html"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a06.md '(_) @n' --json
[
  {
    "n": {
      "address": "a06.md",
      "attrs": {
        "path": "a06.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a06.md",
      "attrs": {
        "name": "a"
      },
      "lines": [
        1,
        5
      ],
      "text": ":::a\n<!--\n:::\n-->\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a06.md",
      "attrs": {},
      "lines": [
        2,
        4
      ],
      "text": "<!--\n:::\n-->",
      "type": "html"
    }
  }
]
```

```scrut
$ cat > a07.md <<'EOF'
> - :::x
>   body
>   :::
> - next
> EOF
```

Probe `a07`:

```scrut
$ henia query --path a07.md '(_) @n' --json
[
  {
    "n": {
      "address": "a07.md",
      "attrs": {
        "file": "a07.md",
        "kind": "document",
        "main": "false",
        "path": "a07.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        4
      ],
      "text": ":::x\n  body\n  :::\n- next",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n  :::",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n  :::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "body",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "next",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "next",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a07.md '(_) @n' --json
[
  {
    "n": {
      "address": "a07.md",
      "attrs": {
        "path": "a07.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        4
      ],
      "text": ":::x\n  body\n  :::\n- next",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n  :::",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n  :::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "body",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "next",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a07.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "next",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a08.md <<'EOF'
> - :::x
>   body
> :::
> EOF
```

Probe `a08`:

```scrut
$ henia query --path a08.md '(_) @n' --json
[
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "file": "a08.md",
        "kind": "document",
        "main": "false",
        "path": "a08.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "kind": "markup",
        "message": "1:3: unclosed-directive: directive x is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:3: unclosed-directive: directive x is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n:::",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n:::",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {},
      "lines": [
        2,
        3
      ],
      "text": "body\n:::",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a08.md '(_) @n' --json
[
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "path": "a08.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "kind": "markup",
        "message": "1:3: unclosed-directive: directive x is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:3: unclosed-directive: directive x is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n:::",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n:::",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n  body\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a08.md",
      "attrs": {},
      "lines": [
        2,
        3
      ],
      "text": "body\n:::",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a09.md <<'EOF'
> > :::x
> > body
> > :::
> EOF
```

Probe `a09`:

```scrut
$ henia query --path a09.md '(_) @n' --json
[
  {
    "n": {
      "address": "a09.md",
      "attrs": {
        "file": "a09.md",
        "kind": "document",
        "main": "false",
        "path": "a09.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a09.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": ":::x\n\u003e body\n\u003e :::",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a09.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n\u003e body\n\u003e :::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a09.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "body",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a09.md '(_) @n' --json
[
  {
    "n": {
      "address": "a09.md",
      "attrs": {
        "path": "a09.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a09.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": ":::x\n> body\n> :::",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a09.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n> body\n> :::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a09.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "body",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a10.md <<'EOF'
> :::x{.a .b #i k=v k=w}
> body
> :::
> EOF
```

Probe `a10`:

```scrut
$ henia query --path a10.md '(_) @n' --json
[
  {
    "n": {
      "address": "a10.md",
      "attrs": {
        "file": "a10.md",
        "kind": "document",
        "main": "false",
        "path": "a10.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a10.md",
      "attrs": {
        "kind": "markup",
        "message": "1:19: duplicate-attribute: attribute \"k\" is repeated"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:19: duplicate-attribute: attribute \"k\" is repeated",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a10.md",
      "attrs": {
        "class": "a b",
        "id": "i",
        "k": "v",
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x{.a .b #i k=v k=w}\nbody\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a10.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "body",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a10.md '(_) @n' --json
[
  {
    "n": {
      "address": "a10.md",
      "attrs": {
        "path": "a10.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a10.md",
      "attrs": {
        "kind": "markup",
        "message": "1:19: duplicate-attribute: attribute \"k\" is repeated"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:19: duplicate-attribute: attribute \"k\" is repeated",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a10.md",
      "attrs": {
        "class": "a b",
        "id": "i",
        "k": "v",
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x{.a .b #i k=v k=w}\nbody\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a10.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "body",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a11.md <<'EOF'
> :::x {bad
> :::
> EOF
```

Probe `a11`:

```scrut
$ henia query --path a11.md '(_) @n' --json
[
  {
    "n": {
      "address": "a11.md",
      "attrs": {
        "file": "a11.md",
        "kind": "document",
        "main": "false",
        "path": "a11.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a11.md",
      "attrs": {
        "kind": "markup",
        "message": "1:10: unclosed directive attributes"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:10: unclosed directive attributes",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a11.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        2
      ],
      "text": ":::x {bad\n:::",
      "type": "directive"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a11.md '(_) @n' --json
[
  {
    "n": {
      "address": "a11.md",
      "attrs": {
        "path": "a11.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a11.md",
      "attrs": {
        "kind": "markup",
        "message": "1:10: unclosed directive attributes"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:10: unclosed directive attributes",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a11.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        2
      ],
      "text": ":::x {bad\n:::",
      "type": "directive"
    }
  }
]
```

```scrut
$ cat > a12.md <<'EOF'
> :::x trailing words
> :::
> EOF
```

Probe `a12`:

```scrut
$ henia query --path a12.md '(_) @n' --json
[
  {
    "n": {
      "address": "a12.md",
      "attrs": {
        "file": "a12.md",
        "kind": "document",
        "main": "false",
        "path": "a12.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a12.md",
      "attrs": {
        "kind": "markup",
        "message": "1:6: invalid directive attributes"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:6: invalid directive attributes",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a12.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        2
      ],
      "text": ":::x trailing words\n:::",
      "type": "directive"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a12.md '(_) @n' --json
[
  {
    "n": {
      "address": "a12.md",
      "attrs": {
        "path": "a12.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a12.md",
      "attrs": {
        "kind": "markup",
        "message": "1:6: invalid directive attributes"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:6: invalid directive attributes",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a12.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        2
      ],
      "text": ":::x trailing words\n:::",
      "type": "directive"
    }
  }
]
```

```scrut
$ cat > a13.md <<'EOF'
> ::leaf[label with `]` code]{a=1}
> EOF
```

Probe `a13`:

```scrut
$ henia query --path a13.md '(_) @n' --json
[
  {
    "n": {
      "address": "a13.md",
      "attrs": {
        "file": "a13.md",
        "kind": "document",
        "main": "false",
        "path": "a13.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a13.md",
      "attrs": {
        "a": "1",
        "name": "leaf"
      },
      "lines": [
        1,
        1
      ],
      "text": "::leaf[label with `]` code]{a=1}",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a13.md",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "]",
      "type": "code"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a13.md '(_) @n' --json
[
  {
    "n": {
      "address": "a13.md",
      "attrs": {
        "path": "a13.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a13.md",
      "attrs": {
        "a": "1",
        "name": "leaf"
      },
      "lines": [
        1,
        1
      ],
      "text": "::leaf[label with `]` code]{a=1}",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a13.md",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "]",
      "type": "code"
    }
  }
]
```

```scrut
$ cat > a14.md <<'EOF'
> ::leaf{a="unterminated}
> EOF
```

Probe `a14`:

```scrut
$ henia query --path a14.md '(_) @n' --json
[
  {
    "n": {
      "address": "a14.md",
      "attrs": {
        "file": "a14.md",
        "kind": "document",
        "main": "false",
        "path": "a14.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a14.md",
      "attrs": {
        "kind": "markup",
        "message": "1:10: attribute \"a\": unclosed quoted value"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:10: attribute \"a\": unclosed quoted value",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a14.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "::leaf{a=\"unterminated}",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a14.md '(_) @n' --json
[
  {
    "n": {
      "address": "a14.md",
      "attrs": {
        "path": "a14.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a14.md",
      "attrs": {
        "kind": "markup",
        "message": "1:10: attribute \"a\": unclosed quoted value"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:10: attribute \"a\": unclosed quoted value",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a14.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "::leaf{a=\"unterminated}",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a15.md <<'EOF'
> ::leaf[x]{a="unterminated}
> EOF
```

Probe `a15`:

```scrut
$ henia query --path a15.md '(_) @n' --json
[
  {
    "n": {
      "address": "a15.md",
      "attrs": {
        "file": "a15.md",
        "kind": "document",
        "main": "false",
        "path": "a15.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a15.md",
      "attrs": {
        "kind": "markup",
        "message": "1:13: attribute \"a\": unclosed quoted value"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:13: attribute \"a\": unclosed quoted value",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a15.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "::leaf[x]{a=\"unterminated}",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a15.md '(_) @n' --json
[
  {
    "n": {
      "address": "a15.md",
      "attrs": {
        "path": "a15.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a15.md",
      "attrs": {
        "kind": "markup",
        "message": "1:13: attribute \"a\": unclosed quoted value"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:13: attribute \"a\": unclosed quoted value",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a15.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "::leaf[x]{a=\"unterminated}",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a16.md <<'EOF'
> text :inl[label :nested[deep]]{k="v"} more :bad[ and :ok[x]{a=1 a=2}
> EOF
```

Probe `a16`:

```scrut
$ henia query --path a16.md '(_) @n' --json
[
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "file": "a16.md",
        "kind": "document",
        "main": "false",
        "path": "a16.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "kind": "markup",
        "message": "1:44: unclosed inline directive\n1:65: duplicate-attribute: attribute \"a\" is repeated"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:44: unclosed inline directive\n1:65: duplicate-attribute: attribute \"a\" is repeated",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "text :inl[label :nested[deep]]{k=\"v\"} more :bad[ and :ok[x]{a=1 a=2}",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "inline": "true",
        "k": "v",
        "name": "inl"
      },
      "lines": [
        1,
        1
      ],
      "text": ":inl[label :nested[deep]]{k=\"v\"}",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "inline": "true",
        "name": "nested"
      },
      "lines": [
        1,
        1
      ],
      "text": ":nested[deep]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "a": "1",
        "inline": "true",
        "name": "ok"
      },
      "lines": [
        1,
        1
      ],
      "text": ":ok[x]{a=1 a=2}",
      "type": "directive"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a16.md '(_) @n' --json
[
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "path": "a16.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "kind": "markup",
        "message": "1:44: unclosed inline directive\n1:65: duplicate-attribute: attribute \"a\" is repeated"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:44: unclosed inline directive\n1:65: duplicate-attribute: attribute \"a\" is repeated",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "text :inl[label :nested[deep]]{k=\"v\"} more :bad[ and :ok[x]{a=1 a=2}",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "inline": "true",
        "k": "v",
        "name": "inl"
      },
      "lines": [
        1,
        1
      ],
      "text": ":inl[label :nested[deep]]{k=\"v\"}",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "inline": "true",
        "name": "nested"
      },
      "lines": [
        1,
        1
      ],
      "text": ":nested[deep]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a16.md",
      "attrs": {
        "a": "1",
        "inline": "true",
        "name": "ok"
      },
      "lines": [
        1,
        1
      ],
      "text": ":ok[x]{a=1 a=2}",
      "type": "directive"
    }
  }
]
```

```scrut
$ cat > a17.md <<'EOF'
> para
> :::x
> in
> :::
> EOF
```

Probe `a17`:

```scrut
$ henia query --path a17.md '(_) @n' --json
[
  {
    "n": {
      "address": "a17.md",
      "attrs": {
        "file": "a17.md",
        "kind": "document",
        "main": "false",
        "path": "a17.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a17.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "para",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a17.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        2,
        4
      ],
      "text": ":::x\nin\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a17.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "in",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a17.md '(_) @n' --json
[
  {
    "n": {
      "address": "a17.md",
      "attrs": {
        "path": "a17.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a17.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "para",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a17.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        2,
        4
      ],
      "text": ":::x\nin\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a17.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "in",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a18.md <<'EOF'
> para
> ::leaf[l]
> more
> EOF
```

Probe `a18`:

```scrut
$ henia query --path a18.md '(_) @n' --json
[
  {
    "n": {
      "address": "a18.md",
      "attrs": {
        "file": "a18.md",
        "kind": "document",
        "main": "false",
        "path": "a18.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a18.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "para",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a18.md",
      "attrs": {
        "name": "leaf"
      },
      "lines": [
        2,
        2
      ],
      "text": "::leaf[l]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a18.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "more",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a18.md '(_) @n' --json
[
  {
    "n": {
      "address": "a18.md",
      "attrs": {
        "path": "a18.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a18.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "para",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a18.md",
      "attrs": {
        "name": "leaf"
      },
      "lines": [
        2,
        2
      ],
      "text": "::leaf[l]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a18.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "more",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a19.md <<'EOF'
> # Head *em* `c` #
> ## Same
> ## Same
> Setext
> multi
> ===
> EOF
```

Probe `a19`:

```scrut
$ henia query --path a19.md '(_) @n' --json
[
  {
    "n": {
      "address": "a19.md",
      "attrs": {
        "file": "a19.md",
        "kind": "document",
        "main": "false",
        "path": "a19.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a19.md#head-em-c",
      "attrs": {
        "id": "head-em-c",
        "level": "1",
        "title": "Head *em* `c`"
      },
      "lines": [
        1,
        3
      ],
      "text": "# Head *em* `c` #\n## Same\n## Same",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#head-em-c",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Head *em* `c`",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a19.md#head-em-c",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "c",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a19.md#same",
      "attrs": {
        "id": "same",
        "level": "2",
        "title": "Same"
      },
      "lines": [
        2,
        2
      ],
      "text": "## Same",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#same",
      "attrs": {
        "level": "2"
      },
      "lines": [
        2,
        2
      ],
      "text": "Same",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a19.md#same-1",
      "attrs": {
        "id": "same-1",
        "level": "2",
        "title": "Same"
      },
      "lines": [
        3,
        3
      ],
      "text": "## Same",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#same-1",
      "attrs": {
        "level": "2"
      },
      "lines": [
        3,
        3
      ],
      "text": "Same",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a19.md#multi",
      "attrs": {
        "id": "multi",
        "level": "1",
        "title": "Setext\nmulti"
      },
      "lines": [
        4,
        5
      ],
      "text": "Setext\nmulti",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#multi",
      "attrs": {
        "level": "1"
      },
      "lines": [
        4,
        5
      ],
      "text": "Setext\nmulti",
      "type": "heading"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a19.md '(_) @n' --json
[
  {
    "n": {
      "address": "a19.md",
      "attrs": {
        "path": "a19.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a19.md#head-em-c",
      "attrs": {
        "id": "head-em-c",
        "level": "1",
        "title": "Head *em* `c`"
      },
      "lines": [
        1,
        3
      ],
      "text": "# Head *em* `c` #\n## Same\n## Same",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#head-em-c",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Head *em* `c`",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a19.md#head-em-c",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "c",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a19.md#same",
      "attrs": {
        "id": "same",
        "level": "2",
        "title": "Same"
      },
      "lines": [
        2,
        2
      ],
      "text": "## Same",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#same",
      "attrs": {
        "level": "2"
      },
      "lines": [
        2,
        2
      ],
      "text": "Same",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a19.md#same-1",
      "attrs": {
        "id": "same-1",
        "level": "2",
        "title": "Same"
      },
      "lines": [
        3,
        3
      ],
      "text": "## Same",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#same-1",
      "attrs": {
        "level": "2"
      },
      "lines": [
        3,
        3
      ],
      "text": "Same",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a19.md#multi",
      "attrs": {
        "id": "multi",
        "level": "1",
        "title": "Setext\nmulti"
      },
      "lines": [
        4,
        5
      ],
      "text": "Setext\nmulti",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a19.md#multi",
      "attrs": {
        "level": "1"
      },
      "lines": [
        4,
        5
      ],
      "text": "Setext\nmulti",
      "type": "heading"
    }
  }
]
```

```scrut
$ cat > a20.md <<'EOF'
> | a | b |
> |---|---|
> | 1 |
> no pipe row
> | x | y | z |
> EOF
```

Probe `a20`:

```scrut
$ henia query --path a20.md '(_) @n' --json
[
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "file": "a20.md",
        "kind": "document",
        "main": "false",
        "path": "a20.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {},
      "lines": [
        1,
        5
      ],
      "text": "| a | b |\n|---|---|\n| 1 |\nno pipe row\n| x | y |",
      "type": "table"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "true",
        "index": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "| a | b |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "a",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "b",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "false",
        "index": "1"
      },
      "lines": [
        3,
        3
      ],
      "text": "| 1 |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        3,
        3
      ],
      "text": "1",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        3,
        3
      ],
      "text": "",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "false",
        "index": "2"
      },
      "lines": [
        4,
        4
      ],
      "text": "no pipe row",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        4,
        4
      ],
      "text": "no pipe row",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        4,
        4
      ],
      "text": "",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "false",
        "index": "3"
      },
      "lines": [
        5,
        5
      ],
      "text": "| x | y |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        5,
        5
      ],
      "text": "x",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        5,
        5
      ],
      "text": "y",
      "type": "cell"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a20.md '(_) @n' --json
[
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "path": "a20.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {},
      "lines": [
        1,
        5
      ],
      "text": "| a | b |\n|---|---|\n| 1 |\nno pipe row\n| x | y |",
      "type": "table"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "true",
        "index": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "| a | b |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "a",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "b",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "false",
        "index": "1"
      },
      "lines": [
        3,
        3
      ],
      "text": "| 1 |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        3,
        3
      ],
      "text": "1",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        3,
        3
      ],
      "text": "",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "false",
        "index": "2"
      },
      "lines": [
        4,
        4
      ],
      "text": "no pipe row",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        4,
        4
      ],
      "text": "no pipe row",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        4,
        4
      ],
      "text": "",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "header": "false",
        "index": "3"
      },
      "lines": [
        5,
        5
      ],
      "text": "| x | y |",
      "type": "row"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "a",
        "index": "0"
      },
      "lines": [
        5,
        5
      ],
      "text": "x",
      "type": "cell"
    }
  },
  {
    "n": {
      "address": "a20.md",
      "attrs": {
        "column": "b",
        "index": "1"
      },
      "lines": [
        5,
        5
      ],
      "text": "y",
      "type": "cell"
    }
  }
]
```

```scrut
$ cat > a21.md <<'EOF'
> [l](<a b> "t") [e](a\_b) [ent](a&amp;b) <foo@bar.com> <http://x.y>
> EOF
```

Probe `a21`:

```scrut
$ henia query --path a21.md '(_) @n' --json
[
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "file": "a21.md",
        "kind": "document",
        "main": "false",
        "path": "a21.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[l](\u003ca b\u003e \"t\") [e](a\\_b) [ent](a\u0026amp;b) \u003cfoo@bar.com\u003e \u003chttp://x.y\u003e",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "dest": "a b",
        "exists": "false",
        "url": "a b"
      },
      "lines": [
        1,
        1
      ],
      "text": "l",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "dest": "a\\_b",
        "exists": "false",
        "url": "a\\_b"
      },
      "lines": [
        1,
        1
      ],
      "text": "e",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "dest": "a\u0026amp;b",
        "exists": "false",
        "url": "a\u0026amp;b"
      },
      "lines": [
        1,
        1
      ],
      "text": "ent",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "dest": "foo@bar.com",
        "exists": "false",
        "url": "foo@bar.com"
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
      "address": "a21.md",
      "attrs": {
        "dest": "http://x.y",
        "url": "http://x.y"
      },
      "lines": [
        1,
        1
      ],
      "text": "",
      "type": "link"
    }
  }
]
```

tela prints the same rows with the AD-20 fixes; the file node carries `path` only.

```scrut
$ tela query --path a21.md '(_) @n' --json
[
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "path": "a21.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[l](<a b> \"t\") [e](a\\_b) [ent](a&amp;b) <foo@bar.com> <http://x.y>",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "url": "a b"
      },
      "lines": [
        1,
        1
      ],
      "text": "l",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "url": "a_b"
      },
      "lines": [
        1,
        1
      ],
      "text": "e",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "url": "a&b"
      },
      "lines": [
        1,
        1
      ],
      "text": "ent",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "url": "foo@bar.com"
      },
      "lines": [
        1,
        1
      ],
      "text": "foo@bar.com",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a21.md",
      "attrs": {
        "url": "http://x.y"
      },
      "lines": [
        1,
        1
      ],
      "text": "http://x.y",
      "type": "link"
    }
  }
]
```

```scrut
$ cat > a22.md <<'EOF'
> - a
> - b
>
> - c
>
> 1. x
> 2) y
> EOF
```

Probe `a22`:

```scrut
$ henia query --path a22.md '(_) @n' --json
[
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "file": "a22.md",
        "kind": "document",
        "main": "false",
        "path": "a22.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        4
      ],
      "text": "a\n- b\n\n- c",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "a",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "a",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "b",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "b",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "c",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "c",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "ordered": "true"
      },
      "lines": [
        6,
        6
      ],
      "text": "x",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        6,
        6
      ],
      "text": "x",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        6,
        6
      ],
      "text": "x",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "ordered": "true"
      },
      "lines": [
        7,
        7
      ],
      "text": "y",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "y",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "y",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a22.md '(_) @n' --json
[
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "path": "a22.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        4
      ],
      "text": "a\n- b\n\n- c",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "a",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "a",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "b",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "b",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "c",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "c",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "ordered": "true"
      },
      "lines": [
        6,
        6
      ],
      "text": "x",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        6,
        6
      ],
      "text": "x",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        6,
        6
      ],
      "text": "x",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {
        "ordered": "true"
      },
      "lines": [
        7,
        7
      ],
      "text": "y",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "y",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a22.md",
      "attrs": {},
      "lines": [
        7,
        7
      ],
      "text": "y",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a23.md <<'EOF'
> 	code
>     more
> EOF
```

Probe `a23`:

```scrut
$ henia query --path a23.md '(_) @n' --json
[
  {
    "n": {
      "address": "a23.md",
      "attrs": {
        "file": "a23.md",
        "kind": "document",
        "main": "false",
        "path": "a23.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a23.md",
      "attrs": {
        "inline": "false"
      },
      "lines": [
        1,
        2
      ],
      "text": "code\n    more",
      "type": "code"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a23.md '(_) @n' --json
[
  {
    "n": {
      "address": "a23.md",
      "attrs": {
        "path": "a23.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a23.md",
      "attrs": {
        "inline": "false"
      },
      "lines": [
        1,
        2
      ],
      "text": "code\n    more",
      "type": "code"
    }
  }
]
```

```scrut
$ cat > a24.md <<'EOF'
> <x>
> text
> </x>
>
> para <span>x</span>
> EOF
```

Probe `a24`:

```scrut
$ henia query --path a24.md '(_) @n' --json
[
  {
    "n": {
      "address": "a24.md",
      "attrs": {
        "file": "a24.md",
        "kind": "document",
        "main": "false",
        "path": "a24.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a24.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "\u003cx\u003e\ntext\n\u003c/x\u003e",
      "type": "html"
    }
  },
  {
    "n": {
      "address": "a24.md",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "para \u003cspan\u003ex\u003c/span\u003e",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a24.md '(_) @n' --json
[
  {
    "n": {
      "address": "a24.md",
      "attrs": {
        "path": "a24.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a24.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "<x>\ntext\n</x>",
      "type": "html"
    }
  },
  {
    "n": {
      "address": "a24.md",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "para <span>x</span>",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a25.md <<'EOF'
> * * *
> ---
> ___
> EOF
```

Probe `a25`:

```scrut
$ henia query --path a25.md '(_) @n' --json
[
  {
    "n": {
      "address": "a25.md",
      "attrs": {
        "file": "a25.md",
        "kind": "document",
        "main": "false",
        "path": "a25.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a25.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "* * *",
      "type": "rule"
    }
  },
  {
    "n": {
      "address": "a25.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "---",
      "type": "rule"
    }
  },
  {
    "n": {
      "address": "a25.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "___",
      "type": "rule"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a25.md '(_) @n' --json
[
  {
    "n": {
      "address": "a25.md",
      "attrs": {
        "path": "a25.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a25.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "* * *",
      "type": "rule"
    }
  },
  {
    "n": {
      "address": "a25.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "---",
      "type": "rule"
    }
  },
  {
    "n": {
      "address": "a25.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "___",
      "type": "rule"
    }
  }
]
```

````scrut
$ cat > a26.md <<'EOF'
> ```
> unclosed
> EOF
````

Probe `a26`:

```scrut
$ henia query --path a26.md '(_) @n' --json
[
  {
    "n": {
      "address": "a26.md",
      "attrs": {
        "file": "a26.md",
        "kind": "document",
        "main": "false",
        "path": "a26.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a26.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        1,
        2
      ],
      "text": "unclosed\n",
      "type": "code"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a26.md '(_) @n' --json
[
  {
    "n": {
      "address": "a26.md",
      "attrs": {
        "path": "a26.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a26.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        1,
        2
      ],
      "text": "unclosed\n",
      "type": "code"
    }
  }
]
```

```scrut
$ cat > a27.md <<'EOF'
> > quote
> lazy
> > - list
> > continued
> EOF
```

Probe `a27`:

```scrut
$ henia query --path a27.md '(_) @n' --json
[
  {
    "n": {
      "address": "a27.md",
      "attrs": {
        "file": "a27.md",
        "kind": "document",
        "main": "false",
        "path": "a27.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        1,
        4
      ],
      "text": "quote\nlazy\n\u003e - list\n\u003e continued",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        1,
        2
      ],
      "text": "quote\nlazy",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        3,
        4
      ],
      "text": "list\n\u003e continued",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        3,
        4
      ],
      "text": "list\n\u003e continued",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        3,
        4
      ],
      "text": "list\n\u003e continued",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a27.md '(_) @n' --json
[
  {
    "n": {
      "address": "a27.md",
      "attrs": {
        "path": "a27.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        1,
        4
      ],
      "text": "quote\nlazy\n> - list\n> continued",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        1,
        2
      ],
      "text": "quote\nlazy",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        3,
        4
      ],
      "text": "list\n> continued",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        3,
        4
      ],
      "text": "list\n> continued",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a27.md",
      "attrs": {},
      "lines": [
        3,
        4
      ],
      "text": "list\n> continued",
      "type": "paragraph"
    }
  }
]
```

````scrut
$ cat > a28.md <<'EOF'
> ``` go extra info
> code
> ```
> ~~~
> ~~~
> EOF
````

Probe `a28`:

```scrut
$ henia query --path a28.md '(_) @n' --json
[
  {
    "n": {
      "address": "a28.md",
      "attrs": {
        "file": "a28.md",
        "kind": "document",
        "main": "false",
        "path": "a28.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a28.md",
      "attrs": {
        "info": "go extra info",
        "inline": "false",
        "lang": "go"
      },
      "lines": [
        1,
        3
      ],
      "text": "code\n",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a28.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        4,
        5
      ],
      "text": "~~~\n~~~",
      "type": "code"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a28.md '(_) @n' --json
[
  {
    "n": {
      "address": "a28.md",
      "attrs": {
        "path": "a28.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a28.md",
      "attrs": {
        "info": "go extra info",
        "inline": "false",
        "lang": "go"
      },
      "lines": [
        1,
        3
      ],
      "text": "code\n",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a28.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        4,
        5
      ],
      "text": "~~~\n~~~",
      "type": "code"
    }
  }
]
```

```scrut
$ cat > a29.md <<'EOF'
> [ref]
>
> [ref]: /url "title"
> ![img](/i.png) [![in](/b)](/c)
> EOF
```

Probe `a29`:

```scrut
$ henia query --path a29.md '(_) @n' --json
[
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "file": "a29.md",
        "kind": "document",
        "main": "false",
        "path": "a29.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[ref]",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "dest": "/url",
        "url": "/url"
      },
      "lines": [
        1,
        1
      ],
      "text": "ref",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "![img](/i.png) [![in](/b)](/c)",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "dest": "/i.png",
        "url": "/i.png"
      },
      "lines": [
        4,
        4
      ],
      "text": "img",
      "type": "image"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "dest": "/c",
        "url": "/c"
      },
      "lines": [
        4,
        4
      ],
      "text": "in",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "dest": "/b",
        "url": "/b"
      },
      "lines": [
        4,
        4
      ],
      "text": "in",
      "type": "image"
    }
  }
]
```

tela prints the same rows with the AD-20 fixes; the file node carries `path` only.

```scrut
$ tela query --path a29.md '(_) @n' --json
[
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "path": "a29.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[ref]",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "url": "/url"
      },
      "lines": [
        1,
        1
      ],
      "text": "ref",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "![img](/i.png) [![in](/b)](/c)",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "url": "/i.png"
      },
      "lines": [
        4,
        4
      ],
      "text": "img",
      "type": "image"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "url": "/c"
      },
      "lines": [
        4,
        4
      ],
      "text": "in",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a29.md",
      "attrs": {
        "url": "/b"
      },
      "lines": [
        4,
        4
      ],
      "text": "in",
      "type": "image"
    }
  }
]
```

```scrut
$ cat > a30.md <<'EOF'
> text with ` `` ` and `` `x` `` and `a
> b`
> EOF
```

Probe `a30`:

```scrut
$ henia query --path a30.md '(_) @n' --json
[
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "file": "a30.md",
        "kind": "document",
        "main": "false",
        "path": "a30.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {},
      "lines": [
        1,
        2
      ],
      "text": "text with ` `` ` and `` `x` `` and `a\nb`",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "inline": "true",
        "ticks": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "``",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "inline": "true",
        "ticks": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "`x`",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        2
      ],
      "text": "a\nb",
      "type": "code"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a30.md '(_) @n' --json
[
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "path": "a30.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {},
      "lines": [
        1,
        2
      ],
      "text": "text with ` `` ` and `` `x` `` and `a\nb`",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "inline": "true",
        "ticks": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "``",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "inline": "true",
        "ticks": "0"
      },
      "lines": [
        1,
        1
      ],
      "text": "`x`",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a30.md",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        2
      ],
      "text": "a\nb",
      "type": "code"
    }
  }
]
```

```scrut
$ cat > a31.md <<'EOF'
> :::x
> :::y
> body
> :::
> EOF
```

Probe `a31`:

```scrut
$ henia query --path a31.md '(_) @n' --json
[
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "file": "a31.md",
        "kind": "document",
        "main": "false",
        "path": "a31.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive x is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive x is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        4
      ],
      "text": ":::x\n:::y\nbody\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "name": "y"
      },
      "lines": [
        2,
        4
      ],
      "text": ":::y\nbody\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "body",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a31.md '(_) @n' --json
[
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "path": "a31.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive x is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive x is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        4
      ],
      "text": ":::x\n:::y\nbody\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {
        "name": "y"
      },
      "lines": [
        2,
        4
      ],
      "text": ":::y\nbody\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a31.md",
      "attrs": {},
      "lines": [
        3,
        3
      ],
      "text": "body",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a32.md <<'EOF'
> :::x
>
>     :::
> :::
> EOF
```

Probe `a32`:

```scrut
$ henia query --path a32.md '(_) @n' --json
[
  {
    "n": {
      "address": "a32.md",
      "attrs": {
        "file": "a32.md",
        "kind": "document",
        "main": "false",
        "path": "a32.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a32.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        4
      ],
      "text": ":::x\n\n    :::\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a32.md",
      "attrs": {
        "inline": "false"
      },
      "lines": [
        3,
        3
      ],
      "text": ":::",
      "type": "code"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a32.md '(_) @n' --json
[
  {
    "n": {
      "address": "a32.md",
      "attrs": {
        "path": "a32.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a32.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        4
      ],
      "text": ":::x\n\n    :::\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a32.md",
      "attrs": {
        "inline": "false"
      },
      "lines": [
        3,
        3
      ],
      "text": ":::",
      "type": "code"
    }
  }
]
```

```scrut
$ cat > a33.md <<'EOF'
> Heading
> ---
> :::d
> ## In dir
> text
> :::
> ## After
> EOF
```

Probe `a33`:

```scrut
$ henia query --path a33.md '(_) @n' --json
[
  {
    "n": {
      "address": "a33.md",
      "attrs": {
        "file": "a33.md",
        "kind": "document",
        "main": "false",
        "path": "a33.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a33.md#heading",
      "attrs": {
        "id": "heading",
        "level": "2",
        "title": "Heading"
      },
      "lines": [
        1,
        6
      ],
      "text": "Heading\n---\n:::d\n## In dir\ntext\n:::",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a33.md#heading",
      "attrs": {
        "level": "2"
      },
      "lines": [
        1,
        1
      ],
      "text": "Heading",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a33.md#heading",
      "attrs": {
        "name": "d"
      },
      "lines": [
        3,
        6
      ],
      "text": ":::d\n## In dir\ntext\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a33.md#in-dir",
      "attrs": {
        "id": "in-dir",
        "level": "2",
        "title": "In dir"
      },
      "lines": [
        4,
        5
      ],
      "text": "## In dir\ntext",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a33.md#in-dir",
      "attrs": {
        "level": "2"
      },
      "lines": [
        4,
        4
      ],
      "text": "In dir",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a33.md#in-dir",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "text",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a33.md#after",
      "attrs": {
        "id": "after",
        "level": "2",
        "title": "After"
      },
      "lines": [
        7,
        7
      ],
      "text": "## After",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a33.md#after",
      "attrs": {
        "level": "2"
      },
      "lines": [
        7,
        7
      ],
      "text": "After",
      "type": "heading"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a33.md '(_) @n' --json
[
  {
    "n": {
      "address": "a33.md",
      "attrs": {
        "path": "a33.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a33.md#heading",
      "attrs": {
        "id": "heading",
        "level": "2",
        "title": "Heading"
      },
      "lines": [
        1,
        6
      ],
      "text": "Heading\n---\n:::d\n## In dir\ntext\n:::",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a33.md#heading",
      "attrs": {
        "level": "2"
      },
      "lines": [
        1,
        1
      ],
      "text": "Heading",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a33.md#heading",
      "attrs": {
        "name": "d"
      },
      "lines": [
        3,
        6
      ],
      "text": ":::d\n## In dir\ntext\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a33.md#in-dir",
      "attrs": {
        "id": "in-dir",
        "level": "2",
        "title": "In dir"
      },
      "lines": [
        4,
        5
      ],
      "text": "## In dir\ntext",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a33.md#in-dir",
      "attrs": {
        "level": "2"
      },
      "lines": [
        4,
        4
      ],
      "text": "In dir",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a33.md#in-dir",
      "attrs": {},
      "lines": [
        5,
        5
      ],
      "text": "text",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a33.md#after",
      "attrs": {
        "id": "after",
        "level": "2",
        "title": "After"
      },
      "lines": [
        7,
        7
      ],
      "text": "## After",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a33.md#after",
      "attrs": {
        "level": "2"
      },
      "lines": [
        7,
        7
      ],
      "text": "After",
      "type": "heading"
    }
  }
]
```

```scrut
$ cat > a34.md <<'EOF'
> - [ ] task
> - [x] done
> EOF
```

Probe `a34`:

```scrut
$ henia query --path a34.md '(_) @n' --json
[
  {
    "n": {
      "address": "a34.md",
      "attrs": {
        "file": "a34.md",
        "kind": "document",
        "main": "false",
        "path": "a34.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        2
      ],
      "text": "[ ] task\n- [x] done",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[ ] task",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[ ] task",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "[x] done",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "[x] done",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a34.md '(_) @n' --json
[
  {
    "n": {
      "address": "a34.md",
      "attrs": {
        "path": "a34.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        2
      ],
      "text": "[ ] task\n- [x] done",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[ ] task",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "[ ] task",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "[x] done",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a34.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "[x] done",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a35.md <<'EOF'
> a  
> b\
> c
> EOF
```

Probe `a35`:

```scrut
$ henia query --path a35.md '(_) @n' --json
[
  {
    "n": {
      "address": "a35.md",
      "attrs": {
        "file": "a35.md",
        "kind": "document",
        "main": "false",
        "path": "a35.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a35.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "a  \nb\\\nc",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a35.md '(_) @n' --json
[
  {
    "n": {
      "address": "a35.md",
      "attrs": {
        "path": "a35.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a35.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "a  \nb\\\nc",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a36.md <<'EOF'
> | `a|b` | c \| d |
> |---|---|
> | [x](y) | *z* |
> EOF
```

Probe `a36`:

```scrut
$ henia query --path a36.md '(_) @n' --json
[
  {
    "n": {
      "address": "a36.md",
      "attrs": {
        "file": "a36.md",
        "kind": "document",
        "main": "false",
        "path": "a36.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a36.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "| `a|b` | c \\| d |\n|---|---|\n| [x](y) | *z* |",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a36.md",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "a|b",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a36.md",
      "attrs": {
        "dest": "y",
        "exists": "false",
        "url": "y"
      },
      "lines": [
        3,
        3
      ],
      "text": "x",
      "type": "link"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a36.md '(_) @n' --json
[
  {
    "n": {
      "address": "a36.md",
      "attrs": {
        "path": "a36.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a36.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "| `a|b` | c \\| d |\n|---|---|\n| [x](y) | *z* |",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a36.md",
      "attrs": {
        "inline": "true",
        "ticks": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "a|b",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a36.md",
      "attrs": {
        "url": "y"
      },
      "lines": [
        3,
        3
      ],
      "text": "x",
      "type": "link"
    }
  }
]
```

```scrut
$ cat > a37.md <<'EOF'
> :x[y]: :z[]{} :w[a]{.c}
> EOF
```

Probe `a37`:

```scrut
$ henia query --path a37.md '(_) @n' --json
[
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "file": "a37.md",
        "kind": "document",
        "main": "false",
        "path": "a37.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": ":x[y]: :z[]{} :w[a]{.c}",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "inline": "true",
        "name": "x"
      },
      "lines": [
        1,
        1
      ],
      "text": ":x[y]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "inline": "true",
        "name": "z"
      },
      "lines": [
        1,
        1
      ],
      "text": ":z[]{}",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "class": "c",
        "inline": "true",
        "name": "w"
      },
      "lines": [
        1,
        1
      ],
      "text": ":w[a]{.c}",
      "type": "directive"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a37.md '(_) @n' --json
[
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "path": "a37.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": ":x[y]: :z[]{} :w[a]{.c}",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "inline": "true",
        "name": "x"
      },
      "lines": [
        1,
        1
      ],
      "text": ":x[y]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "inline": "true",
        "name": "z"
      },
      "lines": [
        1,
        1
      ],
      "text": ":z[]{}",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a37.md",
      "attrs": {
        "class": "c",
        "inline": "true",
        "name": "w"
      },
      "lines": [
        1,
        1
      ],
      "text": ":w[a]{.c}",
      "type": "directive"
    }
  }
]
```

```scrut
$ cat > a38.md <<'EOF'
> # Ünïcode Tïtle 你好
> # 123 !@# heading
> #
> EOF
```

Probe `a38`:

```scrut
$ henia query --path a38.md '(_) @n' --json
[
  {
    "n": {
      "address": "a38.md",
      "attrs": {
        "file": "a38.md",
        "kind": "document",
        "main": "false",
        "path": "a38.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a38.md#ncode-ttle-",
      "attrs": {
        "id": "ncode-ttle-",
        "level": "1",
        "title": "Ünïcode Tïtle 你好"
      },
      "lines": [
        1,
        1
      ],
      "text": "# Ünïcode Tïtle 你好",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a38.md#ncode-ttle-",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Ünïcode Tïtle 你好",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a38.md#123--heading",
      "attrs": {
        "id": "123--heading",
        "level": "1",
        "title": "123 !@# heading"
      },
      "lines": [
        2,
        2
      ],
      "text": "# 123 !@# heading",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a38.md#123--heading",
      "attrs": {
        "level": "1"
      },
      "lines": [
        2,
        2
      ],
      "text": "123 !@# heading",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a38.md#heading",
      "attrs": {
        "id": "heading",
        "level": "1",
        "title": ""
      },
      "lines": [
        1,
        1
      ],
      "text": "",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a38.md#heading",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "",
      "type": "heading"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a38.md '(_) @n' --json
[
  {
    "n": {
      "address": "a38.md",
      "attrs": {
        "path": "a38.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a38.md#ncode-ttle-",
      "attrs": {
        "id": "ncode-ttle-",
        "level": "1",
        "title": "Ünïcode Tïtle 你好"
      },
      "lines": [
        1,
        1
      ],
      "text": "# Ünïcode Tïtle 你好",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a38.md#ncode-ttle-",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Ünïcode Tïtle 你好",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a38.md#123--heading",
      "attrs": {
        "id": "123--heading",
        "level": "1",
        "title": "123 !@# heading"
      },
      "lines": [
        2,
        2
      ],
      "text": "# 123 !@# heading",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a38.md#123--heading",
      "attrs": {
        "level": "1"
      },
      "lines": [
        2,
        2
      ],
      "text": "123 !@# heading",
      "type": "heading"
    }
  },
  {
    "n": {
      "address": "a38.md#heading",
      "attrs": {
        "id": "heading",
        "level": "1",
        "title": ""
      },
      "lines": [
        1,
        1
      ],
      "text": "",
      "type": "section"
    }
  },
  {
    "n": {
      "address": "a38.md#heading",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "",
      "type": "heading"
    }
  }
]
```

````scrut
$ cat > a39.md <<'EOF'
> :::x
> ```
> :::
> EOF
````

Probe `a39`:

````scrut
$ henia query --path a39.md '(_) @n' --json
[
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "file": "a39.md",
        "kind": "document",
        "main": "false",
        "path": "a39.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive x is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive x is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        2,
        3
      ],
      "text": ":::\n",
      "type": "code"
    }
  }
]
````

tela prints the same rows; the file node carries `path` only.

````scrut
$ tela query --path a39.md '(_) @n' --json
[
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "path": "a39.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "kind": "markup",
        "message": "1:1: unclosed-directive: directive x is not closed"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:1: unclosed-directive: directive x is not closed",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        3
      ],
      "text": ":::x\n```\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a39.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        2,
        3
      ],
      "text": ":::\n",
      "type": "code"
    }
  }
]
````

````scrut
$ cat > a40.md <<'EOF'
> > ```
> > code
>
> after
> EOF
````

Probe `a40`:

```scrut
$ henia query --path a40.md '(_) @n' --json
[
  {
    "n": {
      "address": "a40.md",
      "attrs": {
        "file": "a40.md",
        "kind": "document",
        "main": "false",
        "path": "a40.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a40.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "code",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a40.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        1,
        2
      ],
      "text": "code\n",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a40.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "after",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a40.md '(_) @n' --json
[
  {
    "n": {
      "address": "a40.md",
      "attrs": {
        "path": "a40.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a40.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "code",
      "type": "quote"
    }
  },
  {
    "n": {
      "address": "a40.md",
      "attrs": {
        "inline": "false",
        "lang": ""
      },
      "lines": [
        1,
        2
      ],
      "text": "code\n",
      "type": "code"
    }
  },
  {
    "n": {
      "address": "a40.md",
      "attrs": {},
      "lines": [
        4,
        4
      ],
      "text": "after",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a41.md <<'EOF'
> - item
>
>       indented code
> EOF
```

Probe `a41`:

```scrut
$ henia query --path a41.md '(_) @n' --json
[
  {
    "n": {
      "address": "a41.md",
      "attrs": {
        "file": "a41.md",
        "kind": "document",
        "main": "false",
        "path": "a41.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        3
      ],
      "text": "item\n\n      indented code",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "item\n\n      indented code",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "item",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {
        "inline": "false"
      },
      "lines": [
        3,
        3
      ],
      "text": "indented code",
      "type": "code"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a41.md '(_) @n' --json
[
  {
    "n": {
      "address": "a41.md",
      "attrs": {
        "path": "a41.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {
        "ordered": "false"
      },
      "lines": [
        1,
        3
      ],
      "text": "item\n\n      indented code",
      "type": "list"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {},
      "lines": [
        1,
        3
      ],
      "text": "item\n\n      indented code",
      "type": "item"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "item",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a41.md",
      "attrs": {
        "inline": "false"
      },
      "lines": [
        3,
        3
      ],
      "text": "indented code",
      "type": "code"
    }
  }
]
```

```scrut
$ cat > a42.md <<'EOF'
> para
> <div>
> html
> </div>
> EOF
```

Probe `a42`:

```scrut
$ henia query --path a42.md '(_) @n' --json
[
  {
    "n": {
      "address": "a42.md",
      "attrs": {
        "file": "a42.md",
        "kind": "document",
        "main": "false",
        "path": "a42.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a42.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "para",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a42.md",
      "attrs": {},
      "lines": [
        2,
        4
      ],
      "text": "\u003cdiv\u003e\nhtml\n\u003c/div\u003e",
      "type": "html"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a42.md '(_) @n' --json
[
  {
    "n": {
      "address": "a42.md",
      "attrs": {
        "path": "a42.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a42.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "para",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a42.md",
      "attrs": {},
      "lines": [
        2,
        4
      ],
      "text": "<div>\nhtml\n</div>",
      "type": "html"
    }
  }
]
```

```scrut
$ cat > a43.md <<'EOF'
> ::: spaced{a=1}
> x
> :::
> EOF
```

Probe `a43`:

```scrut
$ henia query --path a43.md '(_) @n' --json
[
  {
    "n": {
      "address": "a43.md",
      "attrs": {
        "file": "a43.md",
        "kind": "document",
        "main": "false",
        "path": "a43.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a43.md",
      "attrs": {
        "a": "1",
        "name": "spaced"
      },
      "lines": [
        1,
        3
      ],
      "text": "::: spaced{a=1}\nx\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a43.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "x",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a43.md '(_) @n' --json
[
  {
    "n": {
      "address": "a43.md",
      "attrs": {
        "path": "a43.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a43.md",
      "attrs": {
        "a": "1",
        "name": "spaced"
      },
      "lines": [
        1,
        3
      ],
      "text": "::: spaced{a=1}\nx\n:::",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a43.md",
      "attrs": {},
      "lines": [
        2,
        2
      ],
      "text": "x",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a44.md <<'EOF'
> :::x
>   :::
> EOF
```

Probe `a44`:

```scrut
$ henia query --path a44.md '(_) @n' --json
[
  {
    "n": {
      "address": "a44.md",
      "attrs": {
        "file": "a44.md",
        "kind": "document",
        "main": "false",
        "path": "a44.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a44.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        2
      ],
      "text": ":::x\n  :::",
      "type": "directive"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a44.md '(_) @n' --json
[
  {
    "n": {
      "address": "a44.md",
      "attrs": {
        "path": "a44.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a44.md",
      "attrs": {
        "name": "x"
      },
      "lines": [
        1,
        2
      ],
      "text": ":::x\n  :::",
      "type": "directive"
    }
  }
]
```

```scrut
$ cat > a45.md <<'EOF'
> ::x[unclosed
> EOF
```

Probe `a45`:

```scrut
$ henia query --path a45.md '(_) @n' --json
[
  {
    "n": {
      "address": "a45.md",
      "attrs": {
        "file": "a45.md",
        "kind": "document",
        "main": "false",
        "path": "a45.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a45.md",
      "attrs": {
        "kind": "markup",
        "message": "1:2: unclosed inline directive"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:2: unclosed inline directive",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a45.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "::x[unclosed",
      "type": "paragraph"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a45.md '(_) @n' --json
[
  {
    "n": {
      "address": "a45.md",
      "attrs": {
        "path": "a45.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a45.md",
      "attrs": {
        "kind": "markup",
        "message": "1:2: unclosed inline directive"
      },
      "lines": [
        1,
        1
      ],
      "text": "1:2: unclosed inline directive",
      "type": "problem"
    }
  },
  {
    "n": {
      "address": "a45.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "::x[unclosed",
      "type": "paragraph"
    }
  }
]
```

```scrut
$ cat > a46.md <<'EOF'
> see [link :d[x] here](u) and [*em*](v)
> EOF
```

Probe `a46`:

```scrut
$ henia query --path a46.md '(_) @n' --json
[
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "file": "a46.md",
        "kind": "document",
        "main": "false",
        "path": "a46.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "see [link :d[x] here](u) and [*em*](v)",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "dest": "u",
        "exists": "false",
        "url": "u"
      },
      "lines": [
        1,
        1
      ],
      "text": "link :d[x] here",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "inline": "true",
        "name": "d"
      },
      "lines": [
        1,
        1
      ],
      "text": ":d[x]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "dest": "v",
        "exists": "false",
        "url": "v"
      },
      "lines": [
        1,
        1
      ],
      "text": "em",
      "type": "link"
    }
  }
]
```

tela prints the same rows; the file node carries `path` only.

```scrut
$ tela query --path a46.md '(_) @n' --json
[
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "path": "a46.md"
      },
      "type": "file"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {},
      "lines": [
        1,
        1
      ],
      "text": "see [link :d[x] here](u) and [*em*](v)",
      "type": "paragraph"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "url": "u"
      },
      "lines": [
        1,
        1
      ],
      "text": "link :d[x] here",
      "type": "link"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "inline": "true",
        "name": "d"
      },
      "lines": [
        1,
        1
      ],
      "text": ":d[x]",
      "type": "directive"
    }
  },
  {
    "n": {
      "address": "a46.md",
      "attrs": {
        "url": "v"
      },
      "lines": [
        1,
        1
      ],
      "text": "em",
      "type": "link"
    }
  }
]
```
