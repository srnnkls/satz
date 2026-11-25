//! Generated Markdown: the frontend and the document editor read any of it
//! without panicking, and an unedited document writes it back unchanged.
//! `PROPTEST_CASES` raises the case count.

use proptest::prelude::*;
use proptest::test_runner::Config;

use super::{Document, Markdown};
use crate::frontend::Frontend as _;

const PREFIXES: &[&str] = &[
    " ", "  ", "   ", "    ", "\t", " \t", "  \t", "   \t", "\t ", "> ", ">", ">\t", "- ", "-\t",
    "* ", "+ ", "1. ", "10) ", "2.\t",
];

const BODIES: &[&str] = &[
    "",
    "text",
    "a\tb",
    "# Title",
    "## Title ##",
    "#",
    "===",
    "---",
    "***",
    "```",
    "```js",
    "~~~",
    "~~~ info",
    "````",
    "<div>",
    "</div>",
    "<!-- c -->",
    "[ref]: /url \"t\"",
    "[ref]",
    "[l](u)",
    "![i](p)",
    "<http://x.y>",
    "`code`",
    "``a`b``",
    "*em* _e_",
    ":name[label]{a=b}",
    ":name[open",
    "::leaf",
    "::leaf[x]{k=v}",
    ":::container",
    ":::note{x=1}",
    "::::outer",
    ":::",
    "::::",
    "|a|b|",
    "|-|-|",
    "a | b",
    "{a=\"b\"}",
    "\\",
    "é ü 日本",
    "\0",
    "\u{feff}",
];

const BREAKS: &[&str] = &["\n", "\n", "\r\n", "\n\n", ""];

fn line() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(prop::sample::select(PREFIXES), 0..4),
        prop::collection::vec(prop::sample::select(BODIES), 1..3),
        prop::sample::select(BREAKS),
    )
        .prop_map(|(prefixes, bodies, end)| prefixes.concat() + &bodies.join(" ") + end)
}

fn markdown() -> impl Strategy<Value = String> {
    (
        prop::bool::weighted(0.05),
        prop::collection::vec(line(), 0..12),
    )
        .prop_map(|(bom, lines)| {
            let bom = if bom { "\u{feff}" } else { "" };
            bom.to_owned() + &lines.concat()
        })
}

const FRONTMATTER: &[&str] = &[
    "",
    "---\nkey: v\n---\n",
    "---\r\nkey: [a\r\n---\r\n",
    "---\n",
];

fn file() -> impl Strategy<Value = String> {
    (prop::sample::select(FRONTMATTER), markdown()).prop_map(|(head, body)| head.to_owned() + &body)
}

fn config() -> Config {
    Config {
        failure_persistence: None,
        ..Config::default()
    }
}

fn reads(text: &str) {
    let tree = Markdown::new().parse("doc.md", text);
    for node in tree.root().descendants() {
        if let Some(position) = &node.node().position {
            assert!(position.span().end <= text.len(), "{position:?}");
        }
    }
}

fn written(text: &str) -> String {
    Document::parse("doc.md", text).value.to_string()
}

/// `text` with every run of three or more colons cut to three.
fn fences_cut(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = 0;
    for c in text.chars() {
        run = if c == ':' { run + 1 } else { 0 };
        if run <= 3 {
            out.push(c);
        }
    }
    out
}

/// Byte for byte but for container fences, which take canonical lengths,
/// and NUL, which reads as U+FFFD; written once, the text is canonical and
/// writes back unchanged.
fn round_trips(text: &str) {
    let once = written(text);
    assert_eq!(
        fences_cut(&once),
        fences_cut(&text.replace('\0', "\u{fffd}"))
    );
    assert_eq!(written(&once), once);
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_frontend_reads_any_markdown(text in file()) {
        reads(&text);
    }

    #[test]
    fn an_unedited_document_writes_back_its_source(text in markdown()) {
        round_trips(&text);
    }
}

#[test]
fn nul_reads_as_the_replacement_character() {
    assert_eq!(written("a\0\n"), "a\u{fffd}\n");
}

#[test]
fn a_fence_running_out_of_its_quote_is_written_once() {
    round_trips("> ```\ntext\n");
}

#[test]
fn an_empty_heading_keeps_its_line() {
    round_trips(" ===\n#\n");
}

#[test]
fn a_quote_of_an_empty_heading_keeps_its_line() {
    round_trips("[l](u)\n> #\n\n");
}

#[test]
fn an_empty_quote_keeps_its_line() {
    round_trips("[l](u)\n> \n");
}

#[test]
fn an_empty_item_after_a_rule_keeps_its_marker() {
    round_trips("---\n- \n");
}

#[test]
fn an_item_holding_a_definition_keeps_it() {
    round_trips("- \n  text\n- [ref]: /url \"t\"\n");
}

#[test]
fn an_item_starts_at_its_marker() {
    round_trips(":::container---\n- ```\n\n");
    round_trips("- - \n- - ```\n\n\n");
}

#[test]
fn a_section_starts_at_its_heading() {
    round_trips(":::container\n\n# Title\n> \n");
}

#[test]
fn tabs_count_to_the_next_tab_stop() {
    round_trips("1.   \ttext\n\t# [l](u)\n");
}

#[test]
fn an_item_marked_on_an_indented_quote_line() {
    round_trips("> <!-- c -->\n  > - ```\n");
}

#[test]
fn a_fence_after_a_quote_marker_without_its_space() {
    round_trips("   >\t<!-- c -->\n>~~~\r\n[ref]\n");
}

#[test]
fn a_canonical_fence_outlasts_the_colon_lines_it_holds() {
    round_trips(":::::leaf\n::::\n");
    assert_eq!(
        written(":::::leaf\n::::\n:::::\n"),
        ":::::leaf\n::::\n:::::\n"
    );
}

#[test]
fn a_quoted_heading_without_the_optional_space() {
    round_trips(">\t  \n>[l](u)\n> ===\n");
}
