# tela lint

`tela lint [PATH…]` runs tela's std rules and the inline rules of `tela.toml` over Markdown files and prints findings on stdout. Exit status 0 is clean, 1 is findings (errors, or warnings under `--strict`), and 2 is a usage or config error on stderr (FR-7, AC-6, AC-17).

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
> tela.lint.disable: duplicate-heading
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

Without paths, the current directory is linted. A warning alone exits cleanly; `docs/quiet.md` names `duplicate-heading` under the default disable key `tela.lint.disable`, so it stays silent there.

```scrut
$ tela lint
docs/guide.md:7:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
```

`--strict` fails on warnings too.

```scrut
$ tela lint docs --strict
docs/guide.md:7:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
[1]
```

`--format json` writes each finding as an object.

```scrut
$ tela lint docs --format json
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
$ tela lint docs --strict --disable duplicate-heading
```

## Config

`[lint]` in `tela.toml` disables rules, raises their severity and renames the per-file disable key.

```scrut
$ cat > tela.toml <<'EOF'
> [lint]
> disable = ["duplicate-heading"]
> EOF
```

```scrut
$ tela lint docs --strict
```

```scrut
$ cat > tela.toml <<'EOF'
> [lint.config.duplicate-heading]
> severity = "error"
> EOF
```

```scrut
$ tela lint docs
docs/guide.md:7:4: error [duplicate-heading] heading repeated; first occurrence on line 3
[1]
```

With another disable key, `docs/quiet.md` no longer silences the rule.

```scrut
$ cat > tela.toml <<'EOF'
> [lint]
> disable_key = "lint.off"
> EOF
```

```scrut
$ tela lint docs
docs/guide.md:7:4: warning [duplicate-heading] heading repeated; first occurrence on line 3
docs/quiet.md:11:4: warning [duplicate-heading] heading repeated; first occurrence on line 7
```

An unknown key, an unknown rule and semantic rules without a model are config errors.

```scrut
$ printf '[lint]\nstage = "source"\n' > tela.toml && tela lint docs 2>&1 | tail -1
unknown field `stage`, expected one of `rules`, `config`, `disable`, `disable_key`, `semantic`
```

```scrut
$ printf '[lint]\nstage = "source"\n' > tela.toml && tela lint docs 2>/dev/null
[2]
```

```scrut
$ printf '[lint]\ndisable = ["duplicate-headings"]\n' > tela.toml && tela lint docs 2>&1
tela: no rule duplicate-headings; did you mean duplicate-heading?
[2]
```

```scrut
$ printf '[lint.semantic]\nenabled = true\n' > tela.toml && tela lint docs 2>&1
tela: semantic rules are enabled but no model_path is set
[2]
```

A config file named with `--config` must exist.

```scrut
$ tela lint docs --config other.toml 2>&1
tela: other.toml: no such file or directory
[2]
```

## Examples

`tela lint test` runs every rule's examples.

```scrut
$ : > tela.toml && tela lint test
8 lint examples passed
```
