mod engine;

use super::*;

fn id(id: &str) -> RuleId {
    RuleId::new(id).unwrap()
}

fn layer(package: &str) -> Layer {
    Layer::from_files(Some(package.to_owned()), vec![]).unwrap()
}

#[test]
fn rule_ids_reject_empty_and_spaced_text() {
    assert!(RuleId::new("").is_err());
    assert!(RuleId::new("two words").is_err());
    assert_eq!(
        "duplicate-heading".parse::<RuleId>().map(|r| r.to_string()),
        Ok("duplicate-heading".to_owned())
    );
}

#[test]
fn severities_round_trip_their_names() {
    for severity in [Severity::Error, Severity::Warning] {
        assert_eq!(severity.to_string().parse(), Ok(severity));
    }
    assert!("info".parse::<Severity>().is_err());
}

#[test]
fn two_layers_of_one_package_are_rejected() {
    let result = Config::new(
        vec![layer("team"), layer("team")],
        Spec::default(),
        Settings::default(),
    );
    assert_eq!(
        result,
        Err(ConfigError::DuplicatePackage("team".to_owned()))
    );
}

#[test]
fn semantic_rules_need_a_model() {
    let settings = Settings {
        semantic: Semantic {
            enabled: true,
            model_path: None,
        },
        ..Settings::default()
    };
    assert_eq!(
        Config::new(vec![], Spec::default(), settings),
        Err(ConfigError::MissingModel)
    );
}

#[test]
fn layers_reject_duplicate_files() {
    let file = ModuleFile {
        path: "headings.md".to_owned(),
        text: String::new(),
    };
    assert!(matches!(
        Layer::from_files(None, vec![file.clone(), file]),
        Err(LayerError::DuplicateFile(path)) if path == "headings.md"
    ));
}

#[test]
fn read_dir_collects_markdown_with_relative_paths() {
    let dir = std::env::temp_dir().join(format!("tela-layer-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("team")).unwrap();
    std::fs::write(dir.join("team/headings.md"), "# Headings\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "skip").unwrap();
    let layer = Layer::read_dir(None, &dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(layer.files().len(), 1);
    assert_eq!(layer.files()[0].path, "team/headings.md");
}

#[test]
fn diagnostics_point_at_origins() {
    let at = Origin::new(
        "a.md",
        Some(crate::tree::Position::new(0..5, 14..=14, 1).unwrap()),
    )
    .unwrap();
    let d = Diagnostic::new(
        id("duplicate-heading"),
        at,
        Severity::Warning,
        "heading repeated",
    );
    assert_eq!(
        (d.rule().as_str(), d.origin().line()),
        ("duplicate-heading", Some(14))
    );
}

#[test]
fn scores_carry_their_method_data() {
    let score = Score::Cosine {
        value: 0.98,
        model: "potion-base-2M".to_owned(),
    };
    assert!((score.value() - 0.98).abs() < f64::EPSILON);
}

#[test]
fn selections_filter_rule_ids() {
    let only = Selection::Only(BTreeSet::from([id("a")]));
    assert!(only.includes("a"));
    assert!(!only.includes("b"));
    assert!(Selection::All.includes("b"));
}
