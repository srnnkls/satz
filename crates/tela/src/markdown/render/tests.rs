use super::super::testing::parse;
use super::super::{Document, RenderFormat};
use RenderFormat::{Keep, Markdown, Xml};

/// henia's rendered documents: name, source, format and output.
const RENDERINGS: &[(&str, &str, RenderFormat, &str)] = &[
    (
        "block",
        "# Heading\n\n:::instruction{priority=critical}\nUse **TDD**.\n:::\n",
        Xml,
        "# Heading\n\n<instruction priority=\"critical\">\nUse **TDD**.\n</instruction>\n",
    ),
    (
        "inline",
        "Use :term[TTL]{abbr=\"time to live\"} now.",
        Xml,
        "Use <term abbr=\"time to live\">TTL</term> now.",
    ),
    (
        "nested",
        ":::outer\n:::inner\nx\n:::\n:::\n",
        Xml,
        "<outer>\n<inner>\nx\n</inner>\n</outer>\n",
    ),
    (
        "nested directives",
        ":::outer\n:::inner\nx\n:::\n:::\n",
        Keep,
        ":::outer\n:::inner\nx\n:::\n:::\n",
    ),
    (
        "nested directives attributes",
        "::::outer{a=1}\n:::inner\nUse :term[x]{b=2}.\n:::\n::::\n",
        Keep,
        "::::outer{a=\"1\"}\n:::inner\nUse :term[x]{b=\"2\"}.\n:::\n::::\n",
    ),
    (
        "nested inline",
        ":outer[Use :inner[**text**]{}]",
        Xml,
        "<outer>Use <inner>**text**</inner></outer>",
    ),
    (
        "inline context",
        ":outer[:::literal]",
        Xml,
        "<outer>:::literal</outer>",
    ),
    (
        "CRLF",
        ":::x\r\ntext\r\n:::\r\n",
        Xml,
        "<x>\r\ntext\r\n</x>\r\n",
    ),
    (
        "XML whitespace",
        ":x[y]{a=\"line\\nnext\\tcolumn\"}",
        Xml,
        "<x a=\"line&#xA;next&#x9;column\">y</x>",
    ),
    (
        "brackets",
        ":term[a [link](url) and `]`]{#main .one .two}",
        Xml,
        "<term id=\"main\" class=\"one two\">a [link](url) and `]`</term>",
    ),
    (
        "class key then shorthand",
        ":x[y]{class=a .b}",
        Xml,
        "<x class=\"a b\">y</x>",
    ),
    (
        "class shorthand then key",
        ":x[y]{.a class=b}",
        Xml,
        "<x class=\"a b\">y</x>",
    ),
    (
        "normalize",
        "::: note {key = \"val\"}\nx\n:::\nUse :term[x] {}.",
        Keep,
        ":::note{key=\"val\"}\nx\n:::\nUse :term[x].",
    ),
    (
        "escaped attributes",
        ":x[y]{msg=\"Use \\\"quotes\\\" & <tags>\"}",
        Xml,
        "<x msg=\"Use &#34;quotes&#34; &amp; &lt;tags&gt;\">y</x>",
    ),
    (
        "code",
        "```md\n:::x{bad=}\n:x[bad\n```\n\n`:x[y]`\n\n    :::x\n",
        Xml,
        "```md\n:::x{bad=}\n:x[bad\n```\n\n`:x[y]`\n\n    :::x\n",
    ),
    (
        "code inside directive",
        ":::x\n```\n:::\n```\n:::\n",
        Xml,
        "<x>\n```\n:::\n```\n</x>\n",
    ),
    ("escaped", "\\:::x\n\\:x[y]\n", Xml, "\\:::x\n\\:x[y]\n"),
    (
        "blockquote",
        "> :::x\n> text\n> :::\n",
        Xml,
        "> <x>\n> text\n> </x>\n",
    ),
    (
        "list",
        "- :::x\n  text\n  :::\n",
        Xml,
        "- <x>\n  text\n  </x>\n",
    ),
    ("empty", ":::x\n:::", Xml, "<x>\n</x>"),
    (
        "plain markdown directives",
        ":::note{priority=high}\nUse :term[TTL]{abbr=cache}.\n:::\n",
        Markdown,
        "**note priority=\"high\"**\nUse **term abbr=\"cache\":** TTL.\n\n",
    ),
    (
        "markdown",
        "# Heading\n\n- **bold**  \n  next\n\n[link]: ./file.md\n",
        Xml,
        "# Heading\n\n- **bold**  \n  next\n\n[link]: ./file.md\n",
    ),
    (
        "leaf inside container",
        ":::x\n::y[a]\n:::\n",
        Xml,
        "<x>\n<y>a</y>\n</x>\n",
    ),
    ("unlabeled leaf", "::body\n", Xml, "<body></body>\n"),
];

#[test]
fn documents_render_in_each_format() {
    for &(name, text, format, want) in RENDERINGS {
        let mut doc = parse("skill.md", text);
        let problems = doc.render(format);
        assert!(problems.is_empty(), "{name}: {problems:?}");
        assert_eq!(doc.to_string(), want, "{name}");
    }
}

#[test]
fn markdown_format_quotes_and_escapes_labels() {
    let mut doc = parse("a.md", ":x[y]{a=\"t\\u0085b\" k=\"*_[x]<\"}\n");
    assert!(doc.render(Markdown).is_empty());
    assert_eq!(
        doc.to_string(),
        "**x a=\"t\\u0085b\" k=\"\\*\\_\\[x\\]&lt;\":** y\n"
    );
}

#[test]
fn invalid_markup_is_located_in_the_source() {
    for text in [
        ":::x{attr=}\nx\n:::",
        ":x[y]{key=\"unclosed}",
        ":::x\ntext",
        ":x[unclosed",
        ":::x{a=1 a=2}\n:::",
        ":x[y]{a=[1,2]}",
        ":outer[:inner[x]{a=}]",
        ":x[y]{a=\"\\b\"}",
        "::x{a=}",
        "::x{k=1 k=2 bad=}",
    ] {
        let mut doc = Document::parse("skill.md", &format!("intro\n\n{text}\n")).value;
        let problems = doc.render(Xml);
        assert!(!problems.is_empty(), "{text:?}");
        for problem in &problems {
            assert_eq!(problem.origin.line(), Some(3), "{text:?}: {problem}");
        }
    }
}

#[test]
fn every_failing_block_is_reported_in_source_order() {
    let mut doc = Document::parse("a.md", ":x[y]{a=}\n\nfine\n\n:z[w]{b=}\n").value;
    let lines: Vec<_> = doc.render(Xml).iter().map(|p| p.origin.line()).collect();
    assert_eq!(lines, [Some(1), Some(5)]);
}

#[test]
fn rendering_keep_is_idempotent() {
    for text in [
        ":::x\n:x[y]{a=1}\n:::",
        ":x[`]`]{a=b}",
        "",
        "\\:x[y]",
        "- :::n{#i .c}\n  ::l[t]{k=\"a b\"}\n  :::\n",
    ] {
        let mut doc = parse("a.md", text);
        assert!(doc.render(Keep).is_empty());
        let once = doc.to_string();
        let mut again = parse("a.md", &once);
        assert!(again.render(Keep).is_empty());
        assert_eq!(again.to_string(), once, "{text:?}");
    }
}

#[test]
fn unwrapping_moves_content_out_of_named_directives() {
    let text = "# Skill\n\n:::head\n## Routes\n\n| a | b |\n\n:::instruction{priority=high}\nRule.\n:::\n:::\n\n## Detail\n\nMore.\n\n```md\n:::head\nliteral\n:::\n```\n\n:::head\nLast.\n:::\n";
    let mut doc = parse("skill.md", text);
    let head = doc.unwrap_directive("head");
    assert_eq!(
        doc.to_string(),
        "# Skill\n\n## Routes\n\n| a | b |\n\n:::instruction{priority=high}\nRule.\n:::\n\n## Detail\n\nMore.\n\n```md\n:::head\nliteral\n:::\n```\n\nLast.\n"
    );
    assert_eq!(
        head.to_string(),
        "## Routes\n\n| a | b |\n\n:::instruction{priority=high}\nRule.\n:::\n\nLast.\n"
    );
    assert_eq!(head.file, "skill.md");
}

#[test]
fn unwrapping_an_absent_directive_changes_nothing() {
    let text = "# Skill\n\n:::note\nx\n:::\n";
    let mut doc = parse("skill.md", text);
    let head = doc.unwrap_directive("head");
    assert_eq!(doc.to_string(), text);
    assert!(head.children.is_empty());
}
