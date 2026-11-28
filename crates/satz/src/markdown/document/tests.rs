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
    assert_eq!(generated.origin.file(), "components/card.md");
    assert_eq!(generated.origin.position().map(Position::span), Some(span));
    assert_eq!(generated.origin.to_string(), "components/card.md:3:1");

    generated.expansion.push(Call {
        directive: call,
        definition: generated.origin.clone(),
    });
    host.children.push(generated);

    let out = host.to_string();
    assert!(out.contains("alpha") && out.contains("# Skill"), "{out:?}");
    assert!(out.contains("beta  *keep*\\\n   odd   text"), "{out:?}");
    let grafted = host.children.last().unwrap();
    assert_eq!(grafted.origin.file(), "components/card.md");
    assert_eq!(grafted.expansion.len(), 1);
    assert_eq!(grafted.expansion[0].directive.file(), "skill.md");
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
fn partly_stripped_tabs_are_written_verbatim() {
    for text in [
        "- a\n \t}\n-\tb",
        "- a\n\t}\n- b\n",
        "- a\n  \t}\n- b\n",
        "- a\n   \t}\n- b\n",
        "10. a\n \t}\n11. b\n",
        "- a\n  - b\n   \tc\n  - d\n",
        "- a\n \t- b\n \t \t c\n- d\n",
        "- > a\n  > \tb\n- c\n",
        "- a\n\n  > b\n \t> c\n- d\n",
        "- x\n\n  a\n \tb\n  ===\n- c\n",
        "- a\n \t[l](u) `c`\n- b\n",
        "- a\n \t`x\n \ty` z\n- b\n",
        "- a\n \t:x[y]\n- b\n",
        "- a\n\n \t\tcode\n \t\t  more\n- b\n",
        "- a\n\n \t```\n \t\tx\n \t```\n- b\n",
        "- a\n\n \t<div>\n \tx\n \t</div>\n- b\n",
        "- a\n\n \t:::d\n \tin\n \t:::\n- b\n",
        "- a\r\n \t}\r\n- b\r\n",
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

#[test]
fn leaf_label_keeps_its_text_around_inline_directives() {
    let doc = parse("doc.md", "::wrap[before :inner[] after]\n");
    let leaf = find(&doc, "directive", 0);
    assert_eq!(leaf.attrs.get("name").map(String::as_str), Some("wrap"));
    assert_eq!(leaf.text, "before :inner[] after");
    let children: Vec<_> = leaf
        .children
        .iter()
        .map(|child| {
            (
                child.kind.as_str(),
                child.attrs.get("name").map(String::as_str),
            )
        })
        .collect();
    assert_eq!(children, [("directive", Some("inner"))]);
}
