# satz lint

`satz lint [PATH…]` runs satz's std rules and the inline rules of `satz.toml` over Markdown files and prints findings on stdout. Exit status 0 is clean, 1 is findings (errors, or warnings under `--strict`), and 2 is a usage or config error on stderr (FR-7, AC-6, AC-17).

Only the config written into this file's working directory is read.

```scrut
$ export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/config" XDG_CACHE_HOME="$PWD/cache"
```

```scrut
$ mkdir -p docs && cat > docs/guide.md <<'EOF'
> # Guide
>
> ## Usage
>
> Run the gate.
>
> ## Usage
>
> Run it once more.
> EOF
```

```scrut
$ cat > docs/quiet.md <<'EOF'
> ---
> satz.lint.disable: duplicate-heading
> ---
>
> # Quiet
>
> ## Notes
>
> One.
>
> ## Notes
>
> Two.
> EOF
```

## Findings

Without paths, the current directory is linted. A warning alone exits cleanly; `docs/quiet.md` names `duplicate-heading` under the default disable key `satz.lint.disable`, so it stays silent there.

```scrut
$ satz lint
docs/guide.md:7:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
```

`--strict` fails on warnings too.

```scrut
$ satz lint docs --strict
docs/guide.md:7:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
[1]
```

`--format json` writes each finding as an object.

```scrut
$ satz lint docs --format json
[
  {
    "path": "docs/guide.md",
    "line": 7,
    "column": 4,
    "severity": "warning",
    "rule": "duplicate-heading",
    "message": "heading repeated; first occurrence on line 3",
    "related": [
      {
        "path": "docs/guide.md",
        "line": 3,
        "column": 4
      }
    ]
  }
]
```

`--disable` silences a rule.

```scrut
$ satz lint docs --strict --disable duplicate-heading
```

## Config

`[lint]` in `satz.toml` disables rules, raises their severity and renames the per-file disable key.

```scrut
$ cat > satz.toml <<'EOF'
> [lint]
> disable = ["duplicate-heading"]
> EOF
```

```scrut
$ satz lint docs --strict
```

```scrut
$ cat > satz.toml <<'EOF'
> [lint.config.duplicate-heading]
> severity = "error"
> EOF
```

```scrut
$ satz lint docs
docs/guide.md:7:4: error [duplicate-heading] heading repeated; first occurrence on line 3
[1]
```

With another disable key, `docs/quiet.md` no longer silences the rule.

```scrut
$ cat > satz.toml <<'EOF'
> [lint]
> disable_key = "lint.off"
> EOF
```

```scrut
$ satz lint docs
docs/guide.md:7:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
docs/quiet.md:11:4: warning [duplicate-heading] heading repeated; first occurrence on line 7
```

An unknown key, an unknown rule and semantic rules without a model are config errors.

```scrut
$ printf '[lint]\nstage = "source"\n' > satz.toml && satz lint docs 2>&1 | tail -1
unknown field `stage`, expected one of `rules`, `config`, `disable`, `disable_key`, `semantic`
```

```scrut
$ printf '[lint]\nstage = "source"\n' > satz.toml && satz lint docs 2>/dev/null
[2]
```

```scrut
$ printf '[lint]\ndisable = ["duplicate-headings"]\n' > satz.toml && satz lint docs 2>&1
satz: no rule duplicate-headings; did you mean duplicate-heading?
[2]
```

```scrut
$ printf '[lint.semantic]\nenabled = true\n' > satz.toml && satz lint docs 2>&1
satz: semantic rules are enabled but no model_path is set
[2]
```

A relative `model_path` resolves against the config file's directory, after `$VAR` and `~/` expand; a path that names no directory is a config error.

```scrut
$ mkdir -p conf && printf '[lint.semantic]\nenabled = true\nmodel_path = "models/potion"\n' > conf/lint.toml && satz lint docs --config conf/lint.toml 2>&1
satz: semantic model conf/models/potion: no such directory
[2]
```

```scrut
$ printf '[lint.semantic]\nenabled = true\nmodel_path = "$HOME/models/potion"\n' > satz.toml && satz lint docs 2>&1 | sed "s#$HOME#HOME#"
satz: semantic model HOME/models/potion: no such directory
```

An inline rule whose query does not read points into its query.

```scrut
$ printf '[[lint.rules]]\nid = "bad"\nquery = "(heading) @h (headng) @x"\nmessage = "m"\n' > satz.toml && satz lint docs 2>&1
satz: module "satz.toml", rule bad: unknown type "headng"
  (heading) @h (headng) @x
                ^
  did you mean heading?
[2]
```

A config file named with `--config` must exist.

```scrut
$ satz lint docs --config other.toml 2>&1
satz: other.toml: no such file or directory
[2]
```

## Examples

`satz lint test` runs every rule's examples.

```scrut
$ : > satz.toml && satz lint test
8 lint examples passed
```

## Module layers

Lint modules in the project's `.satz/lint` and the user's `$XDG_CONFIG_HOME/satz/lint` join the std modules; their rules run and their examples are tested.

```scrut
$ mkdir -p .satz/lint config/satz/lint && cat > .satz/lint/build.md <<'EOF'
> ## no-make
>
> ```tq
> (rule no-make :message "use the mise task, not {@c.text}" (code :inline true :matches /^make\b/) @c)
> ```
>
> ### Matches
>
> ```md
> Run `make release`.
> ```
> EOF
```

```scrut
$ cat > config/satz/lint/notes.md <<'EOF'
> ## no-todo
>
> ```tq
> (rule no-todo :message "resolve the TODO" (paragraph :matches /TODO/) @p)
> ```
>
> ### Passes
>
> ```md
> Done.
> ```
> EOF
```

```scrut
$ printf 'Run `make release`.\n\nTODO: tidy.\n' > docs/build.md && satz lint docs/build.md
docs/build.md:1:5: warning [no-make] use the mise task, not make release
docs/build.md:3:1: warning [no-todo] resolve the TODO
```

```scrut
$ satz lint test
10 lint examples passed
```
