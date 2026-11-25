use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crate::frontend::Frontend;
use crate::lint::*;
use crate::markdown::Markdown;
use crate::query::{self, CORPUS, Environment, ErrorKind, Table, Value};
use crate::similarity::{Embed, EmbedError};

fn id(id: &str) -> RuleId {
    RuleId::new(id).unwrap()
}

fn compile(layers: Vec<Layer>, settings: Settings) -> Result<Plan, CompileError> {
    Plan::compile(&Config::new(layers, Spec::default(), settings).unwrap())
}

fn plan(settings: Settings) -> Plan {
    compile(vec![], settings).unwrap()
}

fn module(path: &str, text: &str) -> Layer {
    Layer::from_files(
        None,
        vec![ModuleFile {
            path: path.to_owned(),
            text: text.to_owned(),
        }],
    )
    .unwrap()
}

/// Markdown files with their `path`, as the CLI's walker reads them.
fn corpus(files: &[(&str, &str)]) -> Corpus {
    let markdown = Markdown::new();
    let trees = files.iter().map(|&(path, text)| {
        let mut tree = markdown.parse(path, text);
        let root = tree.root().id();
        let file = tree.node_mut(root).unwrap();
        file.attrs.insert("path".to_owned(), path.to_owned());
        tree
    });
    Corpus::new(trees, Environment::new(UNIX_EPOCH)).unwrap()
}

fn run(plan: &Plan, files: &[(&str, &str)]) -> Vec<Diagnostic> {
    plan.run(&corpus(files), &Selection::All).unwrap()
}

fn listed(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|d| format!("{} {}: {}", d.rule(), d.origin(), d.message()))
        .collect()
}

fn configured(rule: &str, params: &[(&str, Value)], data: &[(&str, Table)]) -> Settings {
    let config = RuleConfig {
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        data: data
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        ..RuleConfig::default()
    };
    Settings {
        config: BTreeMap::from([(id(rule), config)]),
        ..Settings::default()
    }
}

fn outdated() -> Table {
    Table::Map(vec![(
        "legacy-model".to_owned(),
        "current-model".to_owned(),
    )])
}

#[test]
fn std_examples_pass() {
    let plan = plan(Settings::default());
    assert!(!plan.examples().is_empty());
    for example in plan.examples() {
        let outcome = plan.run_example(example, &corpus(&[("example.md", &example.text)]));
        assert_eq!(
            outcome,
            Outcome::Pass,
            "{} line {}",
            example.module,
            example.line
        );
    }
}

#[test]
fn std_rules_come_from_their_modules() {
    let plan = plan(Settings::default());
    let rules: Vec<(&str, &str)> = plan
        .rules()
        .iter()
        .map(|r| (r.id.as_str(), r.module.as_str()))
        .collect();
    assert_eq!(
        rules,
        [
            ("duplicate-content", "std/duplicates"),
            ("outdated-reference", "std/links"),
            ("similar-content", "std/similarity"),
            ("semantic-content", "std/similarity"),
            ("duplicate-heading", "std/structure"),
        ]
    );
}

#[test]
fn examples_type_their_settings_as_the_rule_declares() {
    let plan = plan(Settings::default());
    let example = |rule: &str| {
        plan.examples()
            .iter()
            .find(|e| e.rule.as_str() == rule)
            .unwrap()
    };
    let duplicates = example("duplicate-content");
    assert_eq!((duplicates.line, duplicates.expect), (27, Expect::Matches));
    assert_eq!(duplicates.params["min-words"], Value::Number(3.0));
    assert_eq!(example("outdated-reference").data["outdated"], outdated());
}

const ALPHA: &str = "---
name: alpha
description: Fixture skill on which every tela rule fires.
---

# Alpha

## Usage

The gate runs every formatter, linter and test before a commit lands on the main branch.

The gate runs every formatter, linter and test before a commit lands on the main branch.

The gate runs every formatter, linter and the tests before any commit lands on the main branch today.

Use legacy-model here.

## Usage

Before a change lands on main, the gate executes all formatters, linters and tests first.
";

const BETA: &str = "---
name: beta
description: Repeats a heading but disables duplicate-heading for itself.
henia.lint.disable: duplicate-heading
---

# Beta

## Notes

Short.

## Notes

Shorter.
";

#[test]
fn parity_fixture_findings_match_henia() {
    let mut settings = configured(
        "similar-content",
        &[
            ("similarity", Value::Number(0.3)),
            ("containment", Value::Number(0.8)),
        ],
        &[],
    );
    settings.config.insert(
        id("outdated-reference"),
        RuleConfig {
            data: BTreeMap::from([("outdated".to_owned(), outdated())]),
            ..RuleConfig::default()
        },
    );
    "henia.lint.disable".clone_into(&mut settings.disable_key);
    let found = run(
        &plan(settings),
        &[
            ("skills/alpha/SKILL.md", ALPHA),
            ("skills/beta/SKILL.md", BETA),
        ],
    );
    assert_eq!(
        listed(&found),
        [
            "duplicate-content skills/alpha/SKILL.md:12:1: paragraph duplicates skills/alpha/SKILL.md:10",
            "similar-content skills/alpha/SKILL.md:14:1: paragraph has 42.9% word-shingle jaccard with skills/alpha/SKILL.md:10; shared phrases: commit lands on; every formatter linter; formatter linter and; gate runs every; lands on the",
            "outdated-reference skills/alpha/SKILL.md:16:5: \"legacy-model\" is marked outdated; use \"current-model\"",
            "duplicate-heading skills/alpha/SKILL.md:18:4: heading repeated; first occurrence on line 8",
        ]
    );
    let related: Vec<String> = found
        .iter()
        .map(|d| d.related().iter().map(ToString::to_string).collect())
        .collect();
    assert_eq!(
        related,
        [
            "skills/alpha/SKILL.md:10:1",
            "skills/alpha/SKILL.md:10:1",
            "",
            "skills/alpha/SKILL.md:8:4",
        ]
    );
    let shared = [
        "commit lands on",
        "every formatter linter",
        "formatter linter and",
        "gate runs every",
        "lands on the",
    ];
    assert_eq!(
        found[1].score(),
        Some(&Score::Jaccard {
            value: 0.428_571_428_571_428_55,
            shared: shared.map(str::to_owned).to_vec(),
        })
    );
    assert_eq!(found[0].score(), None);
}

#[derive(Debug)]
struct Soup;

impl Embed for Soup {
    fn model(&self) -> &'static str {
        "soup"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        Ok(if text.contains("soup") {
            vec![0.0, 1.0]
        } else {
            vec![1.0, 0.0]
        })
    }
}

fn semantic(enabled: bool) -> Settings {
    let mut settings = configured(
        "semantic-content",
        &[
            ("min-words", Value::Number(1.0)),
            ("threshold", Value::Number(0.85)),
        ],
        &[],
    );
    settings.config.insert(
        id("duplicate-content"),
        RuleConfig {
            params: BTreeMap::from([("min-words".to_owned(), Value::Number(1.0))]),
            ..RuleConfig::default()
        },
    );
    settings.semantic = Semantic {
        enabled,
        model_path: Some("soup".into()),
    };
    settings
}

#[test]
fn semantic_rules_run_with_a_model_once_enabled() {
    let mut corpus = corpus(&[
        ("a.md", "Repair broken program.\n"),
        ("b.md", "Fix faulty code.\n"),
        ("c.md", "Simmer vegetable soup.\n"),
        ("d.md", "Repair broken program.\n"),
    ]);
    corpus.environment.embed = Some(Arc::new(Soup));
    let found = plan(semantic(true)).run(&corpus, &Selection::All).unwrap();
    assert_eq!(
        listed(&found),
        [
            "semantic-content b.md:1:1: paragraph may express similar instructions to a.md:1 (cosine 1.000); review meaning and constraints",
            "duplicate-content d.md:1:1: paragraph duplicates a.md:1",
        ]
    );
    assert_eq!(
        found[0].score(),
        Some(&Score::Cosine {
            value: 1.0,
            model: "soup".to_owned()
        })
    );
    let lexical = plan(semantic(false)).run(&corpus, &Selection::All).unwrap();
    assert_eq!(listed(&lexical), listed(&found[1..]));
}

#[test]
fn duplicate_directive_content_reports_the_original() {
    let paragraph = "Inspect every changed function and report concrete failures with enough context to reproduce the problem.";
    let first = format!(":::instruction\n{paragraph}\n:::\n");
    let second = format!(
        ":::outer\n:::instruction\n{}\n:::\n:::\n",
        paragraph.to_uppercase()
    );
    let found = run(
        &plan(Settings::default()),
        &[("a.md", &first), ("b.md", &second)],
    );
    assert_eq!(
        listed(&found),
        ["duplicate-content b.md:3:1: paragraph duplicates a.md:2"]
    );
    assert_eq!(found[0].related()[0].to_string(), "a.md:2:1");
}

#[test]
fn duplicate_content_keeps_link_destinations_apart() {
    let text = "Read this detailed guide before proceeding with any changes to the project configuration or dependencies.";
    let a = format!("[{text}](https://example.test/first)\n");
    let b = format!("[{text}](https://example.test/second)\n");
    assert_eq!(
        run(&plan(Settings::default()), &[("a.md", &a), ("b.md", &b)]),
        []
    );
}

#[test]
fn duplicate_content_threshold_and_literal_contexts() {
    let text = "Run the checks first.\n\nRun  the checks\nfirst.\n\n```md\nRun the checks first.\n```\n\nRun {{.checks}} first.\n\nRun {{.checks}} first.\n";
    let files = [("a.md", text)];
    assert_eq!(run(&plan(Settings::default()), &files), []);
    let mut settings = configured(
        "duplicate-content",
        &[("min-words", Value::Number(4.0))],
        &[],
    );
    assert_eq!(
        listed(&run(&plan(settings.clone()), &files)),
        ["duplicate-content a.md:3:1: paragraph duplicates a.md:1"]
    );
    settings.disable.insert(id("duplicate-content"));
    assert_eq!(run(&plan(settings), &files), []);
}

struct Similar<'a> {
    name: &'a str,
    a: &'a str,
    b: &'a str,
    method: Option<&'a str>,
    similarity: f64,
    containment: f64,
    want: f64,
}

#[test]
fn lexical_similarity_scores_edits_and_containment() {
    let base = "Inspect every changed function and report concrete failures with enough context to reproduce the problem.";
    let edited = base.replacen("every", "each", 1);
    let contained = format!(
        "Before opening a pull request, follow these steps. {base} Afterwards, summarize your findings clearly for the reviewer."
    );
    let preceded = format!("Additional setup instructions precede the main task. {base}");
    let cases = [
        Similar {
            name: "edited",
            a: base,
            b: &edited,
            method: Some("jaccard"),
            similarity: 0.7,
            containment: 0.0,
            want: 11.0 / 15.0,
        },
        Similar {
            name: "reordered",
            a: "Inspect every changed function and report concrete failures. Include enough context to reproduce the problem.",
            b: "Include enough context to reproduce the problem. Inspect every changed function and report concrete failures.",
            method: Some("jaccard"),
            similarity: 0.7,
            containment: 0.0,
            want: 11.0 / 15.0,
        },
        Similar {
            name: "contained",
            a: base,
            b: &contained,
            method: Some("containment"),
            similarity: 0.9,
            containment: 1.0,
            want: 1.0,
        },
        Similar {
            name: "contained-reverse",
            a: &preceded,
            b: base,
            method: Some("containment"),
            similarity: 0.0,
            containment: 1.0,
            want: 1.0,
        },
        Similar {
            name: "different",
            a: base,
            b: "Prepare the vegetable broth by simmering chopped carrots and onions together in a large covered pot.",
            method: None,
            similarity: 0.7,
            containment: 0.9,
            want: 0.0,
        },
        Similar {
            name: "paraphrase",
            a: base,
            b: "Examine modified routines, identify reproducible defects, and explain each issue clearly so another developer can investigate it.",
            method: None,
            similarity: 0.7,
            containment: 0.9,
            want: 0.0,
        },
    ];
    for case in cases {
        let name = case.name;
        let settings = configured(
            "similar-content",
            &[
                ("similarity", Value::Number(case.similarity)),
                ("containment", Value::Number(case.containment)),
            ],
            &[],
        );
        let a = format!(":::instruction\n{}\n:::\n", case.a);
        let b = format!(":::instruction\n{}\n:::\n", case.b);
        let found = run(&plan(settings), &[("a.md", &a), ("b.md", &b)]);
        let Some(method) = case.method else {
            assert_eq!(found, [], "{name}");
            continue;
        };
        assert_eq!(found.len(), 1, "{name}: {:?}", listed(&found));
        let d = &found[0];
        let (value, shared) = match (d.score(), method) {
            (Some(Score::Jaccard { value, shared }), "jaccard")
            | (Some(Score::Containment { value, shared }), "containment") => (value, shared),
            other => panic!("{name}: {other:?}"),
        };
        assert!((value - case.want).abs() < 1e-10, "{name}: {value}");
        assert_eq!(d.rule().as_str(), "similar-content");
        assert_eq!(d.origin().to_string(), "b.md:2:1", "{name}");
        assert_eq!(d.related()[0].file().as_ref(), "a.md");
        assert!(
            d.message()
                .contains(&format!("shared phrases: {}", shared[0])),
            "{name}"
        );
    }
}

#[test]
fn lexical_thresholds_and_disabling() {
    let one = "Inspect every changed function and report concrete failures with enough context to reproduce the problem.";
    let a = format!(":::instruction\n{one}\n:::\n");
    let b = format!(
        ":::instruction\n{}\n:::\n",
        one.replacen("every", "each", 1)
    );
    let files = [("a.md", a.as_str()), ("b.md", b.as_str())];
    for params in [
        vec![],
        vec![("similarity", Value::Number(1.0))],
        vec![
            ("similarity", Value::Number(0.7)),
            ("min-words", Value::Number(100.0)),
        ],
    ] {
        let settings = configured("similar-content", &params, &[]);
        assert_eq!(run(&plan(settings), &files), [], "{params:?}");
    }
    let mut settings = configured(
        "similar-content",
        &[("similarity", Value::Number(0.7))],
        &[],
    );
    assert_eq!(run(&plan(settings.clone()), &files).len(), 1);
    settings.disable.insert(id("similar-content"));
    assert_eq!(run(&plan(settings), &files), []);
}

fn inline(rule: &str, query: &str, message: &str) -> InlineRule {
    InlineRule {
        id: id(rule),
        query: query.to_owned(),
        message: message.to_owned(),
        severity: None,
        at: None,
        related: None,
        params: BTreeMap::new(),
        data: BTreeMap::new(),
    }
}

#[test]
fn inline_rules_report_disable_and_take_severity() {
    let file = "---\nname: example\ndescription: Example\n---\n\n:::instruction{priority=low}\nText.\n:::\n\n:::instruction\nmissing\n:::\n\n```md\n:::instruction\n:::\n```\n\n:::instruction{priority=critical}\nTyped.\n:::\n";
    let rule = inline(
        "instruction-priority",
        r#"(directive :name "instruction" :priority (not /^(normal|high|critical)$/)) @d"#,
        "Invalid priority {@d.priority}",
    );
    let mut settings = Settings {
        rules: vec![rule],
        ..Settings::default()
    };
    let files = [("SKILL.md", file)];
    let found = run(&plan(settings.clone()), &files);
    assert_eq!(
        listed(&found),
        [
            "instruction-priority SKILL.md:6:1: Invalid priority low",
            "instruction-priority SKILL.md:10:1: Invalid priority ",
        ]
    );
    assert_eq!(found[0].severity(), Severity::Warning);
    settings.config.insert(
        id("instruction-priority"),
        RuleConfig {
            severity: Some(Severity::Error),
            ..RuleConfig::default()
        },
    );
    assert_eq!(
        run(&plan(settings.clone()), &files)[0].severity(),
        Severity::Error
    );
    settings.disable.insert(id("instruction-priority"));
    assert_eq!(run(&plan(settings), &files), []);
}

const HEADINGS: &str = "---\ndata:\n  banned: [usage]\n---\n\n## repeated-usage\n\n```hq\n(import std/structure)\n(rule repeated-usage\n  :message \"{@h.text} repeats; first on line {@first.line}\"\n  :at @h\n  (repeated-heading @first @h ?text)\n  (row :table banned :value ?text))\n```\n";

#[test]
fn modules_import_std_defines_and_read_their_data() {
    let settings = Settings {
        disable: BTreeSet::from([id("duplicate-heading")]),
        ..Settings::default()
    };
    let plan = compile(vec![module("team/headings.md", HEADINGS)], settings).unwrap();
    let skill = "---\nname: guide\ndescription: A guide skill for tests.\n---\n\n# Guide\n\n## Usage\n\nText.\n\n## Usage\n\nMore.\n";
    assert_eq!(
        listed(&run(&plan, &[("SKILL.md", skill)])),
        ["repeated-usage SKILL.md:12:4: Usage repeats; first on line 8"]
    );
}

#[test]
fn modules_cannot_shadow_a_std_rule() {
    let clash = "## duplicate-heading\n\n```hq\n(rule duplicate-heading :message \"m\" (heading) @h)\n```\n";
    assert_eq!(
        compile(vec![module("team/clash.md", clash)], Settings::default()).unwrap_err(),
        CompileError::DuplicateRule {
            rule: id("duplicate-heading"),
            first: "std/structure".to_owned(),
            second: "team/clash".to_owned(),
        }
    );
}

fn compile_error(settings: Settings) -> CompileError {
    compile(vec![], settings).unwrap_err()
}

fn inline_error(rule: InlineRule) -> CompileError {
    compile_error(Settings {
        rules: vec![rule],
        ..Settings::default()
    })
}

#[test]
fn invalid_inline_rules_fail_to_compile() {
    let CompileError::Query {
        module,
        rule,
        source,
    } = inline_error(inline("bad", "(headng) @h", "m"))
    else {
        panic!("not a query error");
    };
    assert_eq!(
        (
            module.as_str(),
            rule.as_str(),
            source.kind(),
            source.offset()
        ),
        ("tela.toml", "bad", ErrorKind::UnknownType, 1)
    );
    for (rule, message) in [
        (inline("Bad", "(heading) @h", "m"), "lowercase letters"),
        (inline("x", "(heading) @h", "{@y}"), "uses @y"),
    ] {
        let CompileError::Query { source, .. } = inline_error(rule) else {
            panic!("not a query error");
        };
        assert!(source.message().contains(message), "{source}");
    }
    assert_eq!(
        inline_error(inline("duplicate-heading", "(heading) @h", "m")),
        CompileError::DuplicateRule {
            rule: id("duplicate-heading"),
            first: "std/structure".to_owned(),
            second: "tela.toml".to_owned(),
        }
    );
    let mut limit = inline("limit", "(file :lines (> $limit)) @f", "m");
    limit
        .params
        .insert("limit".to_owned(), Value::Text("many".to_owned()));
    assert_eq!(
        inline_error(limit).to_string(),
        "rule limit: param limit must be a number"
    );
}

#[test]
fn invalid_settings_fail_to_compile() {
    let disabled = Settings {
        disable: BTreeSet::from([id("no-such-rule")]),
        ..Settings::default()
    };
    assert_eq!(
        compile_error(disabled).to_string(),
        "no rule no-such-rule; valid: duplicate-content outdated-reference similar-content semantic-content duplicate-heading"
    );
    let mut typo = Settings::default();
    typo.config
        .insert(id("duplicate-conten"), RuleConfig::default());
    assert_eq!(
        compile_error(typo).to_string(),
        "no rule duplicate-conten; did you mean duplicate-content?"
    );
    let text = |text: &str| Value::Text(text.to_owned());
    let cases = [
        (
            configured(
                "duplicate-content",
                &[("min-word", Value::Number(2.0))],
                &[],
            ),
            "rule duplicate-content has no param or data \"min-word\"; did you mean min-words?",
        ),
        (
            configured("similar-content", &[("similarity", text("high"))], &[]),
            "rule similar-content: param similarity must be a number",
        ),
        (
            configured("outdated-reference", &[("outdated", text("x"))], &[]),
            "rule outdated-reference: data outdated must be a table",
        ),
        (
            configured(
                "outdated-reference",
                &[],
                &[("outdated", Table::List(vec!["a".to_owned()]))],
            ),
            "rule outdated-reference: data outdated must be a table",
        ),
        (
            configured(
                "duplicate-content",
                &[],
                &[("min-words", Table::List(vec![]))],
            ),
            "rule duplicate-content: param min-words must be a number",
        ),
    ];
    for (settings, message) in cases {
        assert_eq!(compile_error(settings).to_string(), message);
    }
}

#[test]
fn params_no_module_sets_come_from_the_environment() {
    let settings = Settings {
        rules: vec![inline(
            "limit",
            "(file :lines (> $limit)) @f",
            "over {$limit}",
        )],
        ..Settings::default()
    };
    let plan = plan(settings);
    let only = Selection::Only(BTreeSet::from([id("limit")]));
    let mut corpus = corpus(&[("a.md", "Text.\n")]);
    let error = plan.run(&corpus, &only).unwrap_err();
    assert_eq!(error.rule.as_str(), "limit");
    assert!(matches!(error.source, query::RunError::MissingParam(ref p) if p == "limit"));
    corpus
        .environment
        .params
        .insert("limit".to_owned(), Value::Number(-1.0));
    assert_eq!(
        listed(&plan.run(&corpus, &only).unwrap()),
        ["limit a.md: over -1"]
    );
}

#[test]
fn files_disable_rules_through_the_disable_key() {
    let nested = "---\nname: guide\ndescription: A guide skill for tests.\nhenia:\n  lint:\n    disable: [duplicate-heading]\n---\n\n# Guide\n\n## Usage\n\n## Usage\n";
    let flat = "---\ntela.lint.disable: duplicate-heading\n---\n\n## Usage\n\n## Usage\n";
    let other = "---\nname: other\ndescription: Another skill for tests.\n---\n\n# Other\n\n## Usage\n\n## Usage\n";
    let files = [("guide.md", nested), ("flat.md", flat), ("other.md", other)];
    let reported = |settings: Settings| -> Vec<String> {
        run(&plan(settings), &files)
            .iter()
            .map(|d| d.origin().file().to_string())
            .collect()
    };
    assert_eq!(reported(Settings::default()), ["guide.md", "other.md"]);
    let mut henia = Settings::default();
    "henia.lint.disable".clone_into(&mut henia.disable_key);
    assert_eq!(reported(henia), ["flat.md", "other.md"]);
}

const CONVENTIONS: &str = "---
data:
  axes: [Errors, Effects]
---

## missing-axis

A scope lists every axis in its Conventions table.

~~~hq
(rule missing-axis :message \"Conventions has no {?axis} row\" :at @s
  (row :table axes :value ?axis)
  (file :path ?p (frontmatter :issue_type /^(Initiative|Feature|Task)$/) (section :title \"Conventions\") @s)
  (not (file :path ?p (section :title \"Conventions\" (table (row (cell :column \"Axis\" :text ?axis)))))))
~~~

### Matches

~~~md
---
issue_type: Task
---

## Conventions

| Axis | Rule |
| --- | --- |
| Errors | Wrap them. |
~~~

### Passes

~~~md
---
issue_type: Task
---

## Conventions

| Axis | Rule |
| --- | --- |
| Errors | Wrap them. |
| Effects | Name them. |
~~~

## unchecked-invariant

Every invariant names its check.

~~~hq
(rule unchecked-invariant :message \"invariant has no check\" :at @r
  (section :title \"Invariants\" (table (row (cell :column \"Check\" :text \"\")) @r)))
~~~

### Matches

~~~md
## Invariants

| Invariant | Check |
| --- | --- |
| Lines exact |  |
~~~

### Passes

~~~md
## Invariants

| Invariant | Check |
| --- | --- |
| Lines exact | go test |
~~~
";

#[test]
fn table_rules_hold_through_negation() {
    let plan = compile(
        vec![module("team/conventions.md", CONVENTIONS)],
        Settings::default(),
    )
    .unwrap();
    let examples: Vec<&Example> = plan
        .examples()
        .iter()
        .filter(|e| e.module == "team/conventions")
        .collect();
    assert_eq!(examples.len(), 4);
    for example in examples {
        assert_eq!(
            plan.run_example(example, &corpus(&[("scope.md", &example.text)])),
            Outcome::Pass,
            "{} line {}",
            example.rule,
            example.line
        );
    }
    let partial = "---\nissue_type: Task\n---\n\n# X\n\n## Conventions\n\n| Axis | Rule |\n| --- | --- |\n| Errors | Wrap them. |\n\n## Invariants\n\n| Invariant | Check |\n| --- | --- |\n| Rows bound | go test |\n| Lines exact |  |\n";
    let complete = "---\nissue_type: Feature\n---\n\n# Y\n\n## Conventions\n\n| Axis | Rule |\n| --- | --- |\n| Errors | Wrap them. |\n| Effects | Name them. |\n";
    let only = Selection::Only(BTreeSet::from([
        id("missing-axis"),
        id("unchecked-invariant"),
    ]));
    let files = corpus(&[("x/scope.md", partial), ("y/scope.md", complete)]);
    assert_eq!(
        listed(&plan.run(&files, &only).unwrap()),
        [
            "missing-axis x/scope.md:7:1: Conventions has no Effects row",
            "unchecked-invariant x/scope.md:18:1: invariant has no check",
        ]
    );
}

#[test]
fn failing_examples_report_their_count() {
    let plan = plan(Settings::default());
    let example = plan
        .examples()
        .iter()
        .find(|e| e.rule.as_str() == "duplicate-heading" && e.expect == Expect::Passes)
        .unwrap();
    let repeated = corpus(&[("example.md", "# A\n\n# A\n")]);
    assert_eq!(
        plan.run_example(example, &repeated),
        Outcome::Fail {
            found: 1,
            problem: None
        }
    );
}

#[test]
fn declared_meta_keys_reach_their_rules() {
    let text = "---\nstage:\n  shouting: rendered\n---\n\n## shouting\n\n```hq\n(rule shouting :message \"m\" (heading) @h)\n(rule quiet :message \"m\" (paragraph) @p)\n```\n";
    let spec = Spec {
        meta_keys: BTreeSet::from(["stage".to_owned()]),
        ..Spec::default()
    };
    let config = Config::new(
        vec![module("team/headings.md", text)],
        spec,
        Settings::default(),
    );
    let plan = Plan::compile(&config.unwrap()).unwrap();
    let meta = |rule: &str| {
        plan.rules()
            .iter()
            .find(|r| r.id.as_str() == rule)
            .map(|r| r.meta.clone())
            .unwrap()
    };
    assert_eq!(meta("shouting")["stage"], "rendered");
    assert!(meta("quiet").is_empty());
    assert_eq!(
        compile(vec![module("team/headings.md", text)], Settings::default()).unwrap_err(),
        CompileError::UnknownMetaKey {
            module: "team/headings".to_owned(),
            key: "stage".to_owned()
        }
    );
}

#[test]
fn a_module_defined_by_two_layers_fails_to_compile() {
    let text = "```hq\n(rule shouting :message \"m\" (heading) @h)\n```\n";
    assert_eq!(
        compile(
            vec![module("team/a.md", text), module("team/a.md", text)],
            Settings::default()
        )
        .unwrap_err(),
        CompileError::DuplicateModule("team/a".to_owned())
    );
}

#[test]
fn a_selection_runs_only_its_rules() {
    let packaged = |package: &str, text: &str| {
        Layer::from_files(
            Some(package.to_owned()),
            vec![ModuleFile {
                path: "rules.md".to_owned(),
                text: text.to_owned(),
            }],
        )
        .unwrap()
    };
    let plan = compile(
        vec![
            packaged(
                "one",
                "```hq\n(rule shouting :message \"loud\" (heading) @h)\n```\n",
            ),
            packaged(
                "two",
                "```hq\n(rule talking :message \"talk\" (paragraph) @p)\n```\n",
            ),
        ],
        Settings::default(),
    )
    .unwrap();
    let files = corpus(&[("a.md", "# Title\n\nText.\n")]);
    let only = |rule: &str| {
        let selection = Selection::Only(BTreeSet::from([id(rule)]));
        listed(&plan.run(&files, &selection).unwrap())
    };
    assert_eq!(only("shouting"), ["shouting a.md:1:3: loud"]);
    assert_eq!(only("talking"), ["talking a.md:3:1: talk"]);
    let modules: Vec<&str> = plan
        .rules()
        .iter()
        .map(|r| r.module.as_str())
        .filter(|m| !m.starts_with("std/"))
        .collect();
    assert_eq!(modules, ["one/rules", "two/rules"]);
}

#[test]
fn read_errors_point_into_their_module() {
    let text = "---\ndescription: Broken.\n---\n\n# Broken\n\n```hq\n(rule loud :message \"m\"\n  (headng) @h)\n```\n";
    let CompileError::Read {
        module,
        line,
        source,
    } = compile(vec![module("team/broken.md", text)], Settings::default()).unwrap_err()
    else {
        panic!("not a read error");
    };
    assert_eq!(
        (module.as_str(), line, source.kind(), source.offset()),
        ("team/broken", 9, ErrorKind::UnknownType, 27)
    );
}

#[test]
fn imports_name_existing_modules_without_cycles() {
    let missing = "```hq\n(import team/none)\n```\n";
    let CompileError::Read { source, .. } =
        compile(vec![module("team/a.md", missing)], Settings::default()).unwrap_err()
    else {
        panic!("not a read error");
    };
    assert_eq!(source.message(), "no module \"team/none\"");
    let file = |path: &str, import: &str| ModuleFile {
        path: path.to_owned(),
        text: format!("```hq\n(import {import})\n```\n"),
    };
    let cycle = Layer::from_files(None, vec![file("a.md", "b"), file("b.md", "a")]).unwrap();
    let CompileError::Read { source, .. } = compile(vec![cycle], Settings::default()).unwrap_err()
    else {
        panic!("not a read error");
    };
    assert!(
        source.message().contains("module \"a\" imports itself"),
        "{source}"
    );
}

#[test]
fn malformed_modules_fail_to_compile() {
    for text in ["---\nparams: [1]\n---\n", "---\nname: [broken\n---\n"] {
        assert!(matches!(
            compile(vec![module("team/m.md", text)], Settings::default()),
            Err(CompileError::Malformed { .. })
        ));
    }
}

#[test]
fn a_corpus_holds_its_files_under_one_root() {
    let corpus = corpus(&[("a.md", "A.\n"), ("b.md", "B.\n")]);
    let root = corpus.tree.root();
    assert_eq!(root.node().kind, CORPUS);
    let files: Vec<&str> = root
        .children()
        .map(|file| file.node().attrs["path"].as_str())
        .collect();
    assert_eq!(files, ["a.md", "b.md"]);
}
