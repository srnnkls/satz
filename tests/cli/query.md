# satz query

`satz query PATTERN` prints one row per match on stdout. Errors go to stderr with exit status 2; a run that matches nothing is still clean (FR-7, AC-17).

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
$ satz query '(file) @f'
@f  docs/a.md  file
@f  docs/b.md  file
```

A file named with `--path` that no frontend claims reads as plain text.

```scrut
$ satz query --path notes.txt '(paragraph) @p'
@p  notes.txt  L1  paragraph  first note
@p  notes.txt  L3  paragraph  second note
```

Overlapping paths read each file once, in either order.

```scrut
$ satz query --path docs --path docs/a.md '(file) @f'
@f  docs/a.md  file
@f  docs/b.md  file
```

```scrut
$ satz query --path docs/a.md --path docs '(file) @f'
@f  docs/a.md  file
@f  docs/b.md  file
```

A file that cannot be read is left out and reported on stderr; the query still runs.

```scrut
$ chmod 000 docs/b.md && satz query --path docs '(file) @f' 2>&1 >/dev/null; chmod 644 docs/b.md
satz: docs/b.md: omitted: permission denied
```

```scrut
$ chmod 000 docs/b.md && satz query --path docs '(file) @f' 2>/dev/null; chmod 644 docs/b.md
@f  docs/a.md  file
```

A missing path is an error.

```scrut
$ satz query --path missing '(file) @f' 2>&1
satz: missing: no such file or directory
[2]
```

## Output

The listing shows each capture's address, lines, type and first line.

```scrut
$ satz query '(heading :level 2) @h'
@h  docs/a.md#usage  L3  heading  Usage
@h  docs/a.md#usage-1  L7  heading  Usage
```

`--text` prints each node's whole text after its line.

```scrut
$ satz query --path docs/b.md '(paragraph) @p' --text
@p  docs/b.md#beta  L3  paragraph  Short.
Short.

```

`--count` prints the number of rows.

```scrut
$ satz query '(heading) @h' --count
4
```

`--limit` keeps the first rows and says on stderr how many it left out.

```scrut
$ satz query '(heading) @h' --limit 1 2>&1
@h  docs/a.md#alpha  L1  heading  Alpha
satz: 1 of 4 rows shown
```

`--sort` orders rows by a variable or capture, descending with a leading `-`; the listing shows the sort variable.

```scrut
$ satz query '(heading :level ?l) @h' --sort '-?l' --limit 2 2>/dev/null
?l=2
@h  docs/a.md#usage  L3  heading  Usage

?l=2
@h  docs/a.md#usage-1  L7  heading  Usage
```

`--json` prints rows as objects keyed by capture, with variables as `?name`.

```scrut
$ satz query --path docs/b.md '(heading :level ?l) @h' --sort ?l --json
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
$ satz query '(table) @t' --json
[]
```

```scrut
$ satz query '(table) @t' 2>&1
satz: no matches
```

The output flags exclude each other.

```scrut
$ satz query '(heading) @h' --json --count 2>/dev/null
[2]
```

## Errors

A query the reader rejects is explained on stderr, pointing at the offset, with exit status 2 and nothing on stdout.

```scrut
$ satz query '(headng) @h' 2>&1 >/dev/null
satz: unknown type "headng"
  (headng) @h
   ^
  did you mean heading?
[2]
```

```scrut
$ satz query '(headng) @h' 2>/dev/null
[2]
```

`satz query` registers no unit, so a relation does not read.

```scrut
$ satz query '(link (to (section) @t)) @l' 2>&1
satz: (to ...) relates units, and the schema has none
  (link (to (section) @t)) @l
         ^
  register a unit in the schema, or match links with (link :url ...)
[2]
```

`(similar …)` needs a model from `--model` or `[lint.semantic] model_path`.

```scrut
$ satz query '(paragraph :text ?t) @a (paragraph :text (similar ?t 0.5 @s)) @b' 2>&1
satz: (similar …) needs a semantic model; pass --model or set [lint.semantic] model_path
[2]
```

An unknown flag is a usage error.

```scrut
$ satz query '(file) @f' --colour 2>/dev/null
[2]
```

## Ignored files

Inside a Git repository, a walk skips what Git ignores; `node_modules` and `vendor` are skipped everywhere.

```scrut
$ mkdir -p vendor && printf '# Vendored\n' > vendor/v.md && git init -q && printf 'docs/b.md\n' > .gitignore && satz query '(file) @f'
@f  docs/a.md  file
```
