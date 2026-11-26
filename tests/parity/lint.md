# Lint parity

A skill fixture on which every satz std rule fires, and a second skill that turns `duplicate-heading` off for itself through the per-file disable key, linted by the pinned henia with its own rules disabled and by satz under the same rule config (AD-25, AC-13). henia lints only skills, hence the `skills/` layout.

Both CLIs read only the config written into this file's working directory.

```scrut
$ export HOME="$PWD/home" XDG_CONFIG_HOME="$PWD/config" XDG_CACHE_HOME="$PWD/cache" XDG_DATA_HOME="$PWD/data" XDG_STATE_HOME="$PWD/state" HENIA_HARNESS=none
```

```scrut
$ cat > henia.toml <<EOF
> [lint]
> disable = ["duplicate-skill", "large-skill", "large-head", "invalid-slot", "unused-slot", "unprovided-slot", "unknown-slot", "missing-section", "metadata", "stale-review", "invalid-markup", "broken-link", "missing-reference"]
>
> [lint.config.similar-content]
> similarity = 0.3
> containment = 0.8
>
> [lint.config.semantic-content]
> threshold = 0.7
>
> [lint.config.outdated-reference.outdated]
> legacy-model = "current-model"
>
> [lint.semantic]
> enabled = true
> model_path = "$PARITY_MODEL"
> EOF
```

```scrut
$ cat > satz.toml <<EOF
> [lint]
> disable_key = "henia.lint.disable"
>
> [lint.config.similar-content]
> similarity = 0.3
> containment = 0.8
>
> [lint.config.semantic-content]
> threshold = 0.7
>
> [lint.config.outdated-reference.outdated]
> legacy-model = "current-model"
>
> [lint.semantic]
> enabled = true
> model_path = "$PARITY_MODEL"
> EOF
```

```scrut
$ mkdir -p skills/alpha && cat > skills/alpha/SKILL.md <<'EOF'
> ---
> name: alpha
> description: Fixture skill on which every satz rule fires.
> ---
>
> # Alpha
>
> ## Usage
>
> The gate runs every formatter, linter and test before a commit lands on the main branch.
>
> The gate runs every formatter, linter and test before a commit lands on the main branch.
>
> The gate runs every formatter, linter and the tests before any commit lands on the main branch today.
>
> Use legacy-model here.
>
> ## Usage
>
> Before a change lands on main, the gate executes all formatters, linters and tests first.
> EOF
```

```scrut
$ mkdir -p skills/beta && cat > skills/beta/SKILL.md <<'EOF'
> ---
> name: beta
> description: Repeats a heading but disables duplicate-heading for itself.
> henia.lint.disable: duplicate-heading
> ---
>
> # Beta
>
> ## Notes
>
> Short.
>
> ## Notes
>
> Shorter.
> EOF
```

henia's findings; its messages carry the absolute path of the related file:

```scrut
$ henia lint --format json
[
  {
    "path": "skills/alpha/SKILL.md",
    "line": 12,
    "column": 1,
    "severity": "warning",
    "rule": "duplicate-content",
    "message": "paragraph duplicates */skills/alpha/SKILL.md:10", (glob)
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 10,
        "column": 1
      }
    ],
    "stage": "source"
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 14,
    "column": 1,
    "severity": "warning",
    "rule": "semantic-content",
    "message": "paragraph may express similar instructions to */skills/alpha/SKILL.md:10 (cosine 0.982); review meaning and constraints", (glob)
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 10,
        "column": 1
      }
    ],
    "similarity": 0.9819888423909093,
    "method": "cosine",
    "model": "*/satz/models/potion-base-2M-389b9f6", (glob)
    "stage": "source"
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 14,
    "column": 1,
    "severity": "warning",
    "rule": "similar-content",
    "message": "paragraph has 42.9% word-shingle jaccard with */skills/alpha/SKILL.md:10; shared phrases: commit lands on; every formatter linter; formatter linter and; gate runs every; lands on the", (glob)
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 10,
        "column": 1
      }
    ],
    "similarity": 0.42857142857142855,
    "method": "jaccard",
    "shared_phrases": [
      "commit lands on",
      "every formatter linter",
      "formatter linter and",
      "gate runs every",
      "lands on the"
    ],
    "stage": "source"
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 16,
    "column": 5,
    "severity": "warning",
    "rule": "outdated-reference",
    "message": "\"legacy-model\" is marked outdated; use \"current-model\"",
    "stage": "rendered"
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 18,
    "column": 4,
    "severity": "warning",
    "rule": "duplicate-heading",
    "message": "heading repeated; first occurrence on line 8",
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 8,
        "column": 4
      }
    ],
    "stage": "rendered"
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 20,
    "column": 1,
    "severity": "warning",
    "rule": "semantic-content",
    "message": "paragraph may express similar instructions to */skills/alpha/SKILL.md:10 (cosine 0.743); review meaning and constraints", (glob)
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 10,
        "column": 1
      }
    ],
    "similarity": 0.7430853956418999,
    "method": "cosine",
    "model": "*/satz/models/potion-base-2M-389b9f6", (glob)
    "stage": "source"
  }
]
```

satz's findings are henia's without `stage`, with corpus-relative paths in messages, and with cosine scores to six decimals (AD-25). One finding diverges on purpose: satz drops a `semantic-content` finding where `similar-content` already reports the paragraph under its own config, so line 14 is reported once, as `similar-content`:

```scrut
$ satz lint --format json
[
  {
    "path": "skills/alpha/SKILL.md",
    "line": 12,
    "column": 1,
    "severity": "warning",
    "rule": "duplicate-content",
    "message": "paragraph duplicates skills/alpha/SKILL.md:10",
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 10,
        "column": 1
      }
    ]
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 14,
    "column": 1,
    "severity": "warning",
    "rule": "similar-content",
    "message": "paragraph has 42.9% word-shingle jaccard with skills/alpha/SKILL.md:10; shared phrases: commit lands on; every formatter linter; formatter linter and; gate runs every; lands on the",
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 10,
        "column": 1
      }
    ],
    "similarity": 0.42857142857142855,
    "method": "jaccard",
    "shared_phrases": [
      "commit lands on",
      "every formatter linter",
      "formatter linter and",
      "gate runs every",
      "lands on the"
    ]
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 16,
    "column": 5,
    "severity": "warning",
    "rule": "outdated-reference",
    "message": "\"legacy-model\" is marked outdated; use \"current-model\""
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 18,
    "column": 4,
    "severity": "warning",
    "rule": "duplicate-heading",
    "message": "heading repeated; first occurrence on line 8",
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 8,
        "column": 4
      }
    ]
  },
  {
    "path": "skills/alpha/SKILL.md",
    "line": 20,
    "column": 1,
    "severity": "warning",
    "rule": "semantic-content",
    "message": "paragraph may express similar instructions to skills/alpha/SKILL.md:10 (cosine 0.743); review meaning and constraints",
    "related": [
      {
        "path": "skills/alpha/SKILL.md",
        "line": 10,
        "column": 1
      }
    ],
    "similarity": 0\.743085\d*, (regex)
    "method": "cosine",
    "model": "*/satz/models/potion-base-2M-389b9f6" (glob)
  }
]
```

Three lint behaviours diverge from henia on purpose and have no case here, since henia would report differently: a disable-key value of comma-separated ids such as `duplicate-heading, outdated-reference` turns off each rule, where henia reads it as one id; `:focus` finds its variable case-insensitively, as `:contains` matched it, where henia searches case-sensitively and falls back to the node's start; and `semantic-content` drops the finding above.
