use super::super::testing::{find, item, parse};
use super::super::{Call, Document, Markdown};
use crate::frontend::Frontend as _;
use crate::tree::Position;

#[test]
fn grafted_subtree_keeps_origin_and_expansion() {
    let mut host = parse("skill.md", "# Skill\n\nalpha\n\n::card{x=1}\n");
    let component_source = "intro\n\nbeta  *keep*\\\n   odd   text\n";
    let component = parse("components/card.md", component_source);

    let call = find(&host, "directive", 0).origin.clone();
    let mut generated = find(&component, "paragraph", 1).clone();
    assert_eq!(call.to_string(), "skill.md:5:1");
    assert_eq!(call.position().map(Position::span), Some(16..27));
    let span = 7..7 + "beta  *keep*\\\n   odd   text".len();
    assert_eq!(&**generated.origin.file(), "components/card.md");
    assert_eq!(generated.origin.position().map(Position::span), Some(span));
    assert_eq!(generated.origin.to_string(), "components/card.md:3:1");

    generated.expansion.0.push(Call {
        directive: call,
        component: generated.origin.clone(),
    });
    host.children.push(generated);

    let out = host.to_string();
    assert!(out.contains("alpha") && out.contains("# Skill"), "{out:?}");
    assert!(out.contains("beta  *keep*\\\n   odd   text"), "{out:?}");
    let grafted = host.children.last().unwrap();
    assert_eq!(&**grafted.origin.file(), "components/card.md");
    assert_eq!(grafted.expansion.0.len(), 1);
    assert_eq!(&**grafted.expansion.0[0].directive.file(), "skill.md");
}

#[test]
fn parse_owns_its_source() {
    let mut text = String::from("alpha\n\n::card{x=1}\n");
    let doc = parse("skill.md", &text);
    text.replace_range(..5, "omega");
    assert_eq!(doc.to_string(), "alpha\n\n::card{x=1}\n");
}

#[test]
fn parse_agrees_with_the_tree_on_blocks() {
    let text = "A\n\n---\n\n- a\n\n> q\n\n<div>\nx\n</div>\n\n```\nc\n```\n";
    let tree = Markdown::new().parse("doc.md", text);
    let doc = parse("doc.md", text);
    let blocks: Vec<_> = tree
        .root()
        .children()
        .map(|n| n.node().kind.clone())
        .collect();
    let items: Vec<_> = doc.children.iter().map(|item| item.kind.clone()).collect();
    assert_eq!(items, blocks);
}

#[test]
fn unclosed_empty_directive_in_an_item() {
    let text = "- :::note\nb\n  :::\n";
    let parsed = Document::parse("doc.md", text);
    assert!(
        parsed
            .problems
            .iter()
            .any(|p| p.message.starts_with("unclosed-directive")),
        "{:?}",
        parsed.problems
    );
    assert_eq!(parsed.value.to_string(), text);
}

#[test]
fn edge_inputs_are_written_verbatim() {
    for text in [
        "- :::note\nb\n  :::\n",
        ">#\n000000000000",
        "#\n",
        "# a ##\n\nb\n",
    ] {
        assert_eq!(Document::parse("doc.md", text).value.to_string(), text);
    }
}

#[test]
fn problems_name_their_source_position() {
    let parsed = Document::parse("doc.md", "intro\n\n:x[a]{k=1 k=2}\n");
    let problems: Vec<_> = parsed.problems.iter().map(ToString::to_string).collect();
    assert_eq!(
        problems,
        ["doc.md:3:11: duplicate-attribute: attribute \"k\" is repeated"]
    );
}

#[test]
fn constructed_documents_write_their_blocks() {
    let mut doc = Document::new("doc.md");
    assert_eq!(doc.to_string(), "");
    doc.children.push(item("paragraph", "alpha"));
    assert_eq!(doc.to_string(), "alpha\n");
}
