# tela query

`tela query PATTERN` prints one row per match on stdout. Errors go to stderr with exit status 2; a run that matches nothing is still clean (FR-7, AC-17).

Only the config written into this file's working directory is read.

```scrut
$ export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/config" XDG_CACHE_HOME="$PWD/cache"
```

```scrut
$ mkdir -p docs .drafts && cat > docs/a.md <<'EOF'
> # Alpha
>
> ## Usage
>
> Run the gate.
>
> ## Usage
>
> Run it again.
> EOF
```

```scrut
$ cat > docs/b.md <<'EOF'
> # Beta
>
> Short.
> EOF
```

```scrut
$ printf '# Draft\n' > .drafts/c.md && printf 'first note\n\nsecond note\n' > notes.txt
```

## Files

Without `--path`, the current directory is walked: Markdown files in path order, hidden directories skipped, and `notes.txt` left out since no frontend claims `txt`.

```scrut
$ tela query '(file) @f'
@f  docs/a.md  file
@f  docs/b.md  file
```

A file named with `--path` that no frontend claims reads as plain text.

```scrut
$ tela query --path notes.txt '(paragraph) @p'
@p  notes.txt  L1  paragraph  first note
@p  notes.txt  L3  paragraph  second note
```

Overlapping paths read each file once, in either order.

```scrut
$ tela query --path docs --path docs/a.md '(file) @f'
@f  docs/a.md  file
@f  docs/b.md  file
```

```scrut
$ tela query --path docs/a.md --path docs '(file) @f'
@f  docs/a.md  file
@f  docs/b.md  file
```

A file that cannot be read is left out and reported on stderr; the query still runs.

```scrut
$ chmod 000 docs/b.md && tela query --path docs '(file) @f' 2>&1 >/dev/null; chmod 644 docs/b.md
tela: docs/b.md: omitted: permission denied
```

```scrut
$ chmod 000 docs/b.md && tela query --path docs '(file) @f' 2>/dev/null; chmod 644 docs/b.md
@f  docs/a.md  file
```

A missing path is an error.

```scrut
$ tela query --path missing '(file) @f' 2>&1
tela: missing: no such file or directory
[2]
```

## Output

The listing shows each capture's address, lines, type and first line.

```scrut
$ tela query '(heading :level 2) @h'
@h  docs/a.md#usage  L3  heading  Usage
@h  docs/a.md#usage-1  L7  heading  Usage
```

`--text` prints each node's whole text after its line.

```scrut
$ tela query --path docs/b.md '(paragraph) @p' --text
@p  docs/b.md#beta  L3  paragraph  Short.
Short.

```

`--count` prints the number of rows.

```scrut
$ tela query '(heading) @h' --count
4
```

`--limit` keeps the first rows and says on stderr how many it left out.

```scrut
$ tela query '(heading) @h' --limit 1 2>&1
@h  docs/a.md#alpha  L1  heading  Alpha
tela: 1 of 4 rows shown
```

`--sort` orders rows by a variable or capture, descending with a leading `-`; the listing shows the sort variable.

```scrut
$ tela query '(heading :level ?l) @h' --sort '-?l' --limit 2 2>/dev/null
?l=2
@h  docs/a.md#usage  L3  heading  Usage

?l=2
@h  docs/a.md#usage-1  L7  heading  Usage
```

`--json` prints rows as objects keyed by capture, with variables as `?name`.

```scrut
$ tela query --path docs/b.md '(heading :level ?l) @h' --sort ?l --json
[
  {
    "?l": "1",
    "h": {
      "address": "docs/b.md#beta",
      "attrs": {
        "level": "1"
      },
      "lines": [
        1,
        1
      ],
      "text": "Beta",
      "type": "heading"
    }
  }
]
```

A query without matches prints an empty JSON array, or nothing in a listing, and exits cleanly.

```scrut
$ tela query '(table) @t' --json
[]
```

```scrut
$ tela query '(table) @t' 2>&1
tela: no matches
```

The output flags exclude each other.

```scrut
$ tela query '(heading) @h' --json --count 2>/dev/null
[2]
```

## Errors

A query the reader rejects is explained on stderr, pointing at the offset, with exit status 2 and nothing on stdout.

```scrut
$ tela query '(headng) @h' 2>&1 >/dev/null
tela: unknown type "headng"
  (headng) @h
   ^
  did you mean heading?
[2]
```

```scrut
$ tela query '(headng) @h' 2>/dev/null
[2]
```

`tela query` registers no unit, so a relation does not read.

```scrut
$ tela query '(link (to (section) @t)) @l' 2>&1
tela: (to ...) relates units, and the schema has none
  (link (to (section) @t)) @l
         ^
  register a unit in the schema, or match links with (link :url ...)
[2]
```

`(similar …)` needs a model from `--model` or `[lint.semantic] model_path`.

```scrut
$ tela query '(paragraph :text ?t) @a (paragraph :text (similar ?t 0.5 @s)) @b' 2>&1
tela: (similar …) needs a semantic model; pass --model or set [lint.semantic] model_path
[2]
```

An unknown flag is a usage error.

```scrut
$ tela query '(file) @f' --colour 2>/dev/null
[2]
```

## Ignored files

Inside a Git repository, a walk skips what Git ignores; `node_modules` and `vendor` are skipped everywhere.

```scrut
$ mkdir -p vendor && printf '# Vendored\n' > vendor/v.md && git init -q && printf 'docs/b.md\n' > .gitignore && tela query '(file) @f'
@f  docs/a.md  file
```
