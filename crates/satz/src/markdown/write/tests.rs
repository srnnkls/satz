use super::super::testing::{children_mut, find, find_mut, item, parse};
use super::super::{Document, Item};

type Mutation = Box<dyn Fn(&mut Document)>;

fn check(cases: Vec<(&str, &str, Mutation, &str)>) {
    for (name, text, mutate, want) in cases {
        let mut doc = parse("doc.md", text);
        mutate(&mut doc);
        assert_eq!(doc.to_string(), want, "{name}");
    }
}

/// A structural edit addressed by index paths.
#[derive(Clone, Copy)]
enum Edit {
    Remove(&'static [usize]),
    Reorder(&'static [usize], &'static [usize]),
    ReorderList(&'static [usize]),
}

impl Edit {
    fn apply(self, doc: &mut Document) {
        match self {
            Self::Remove(path) => {
                let (last, parent) = path.split_last().unwrap();
                children_mut(doc, parent).remove(*last);
            }
            Self::Reorder(path, order) => {
                let items = children_mut(doc, path);
                *items = order.iter().map(|&i| items[i].clone()).collect();
            }
            Self::ReorderList(order) => {
                let list = find_mut(doc, "list", 0);
                list.children = order.iter().map(|&i| list.children[i].clone()).collect();
            }
        }
    }
}

const fn without(path: &'static [usize]) -> Edit {
    Edit::Remove(path)
}

const fn reorder_in(path: &'static [usize], order: &'static [usize]) -> Edit {
    Edit::Reorder(path, order)
}

const fn reorder_list(order: &'static [usize]) -> Edit {
    Edit::ReorderList(order)
}

fn append_to(path: &'static [usize], new: Item) -> Mutation {
    Box::new(move |doc| children_mut(doc, path).push(new.clone()))
}

#[test]
fn unchanged_documents_are_verbatim() {
    let texts = [
        "# Title\n\nSome  *odd*   spacing\\\nand a hard break.\n\n::::outer{#top .wide flag}\nIntro :badge[new]{tone=info}.\n\n:::inner{x=\"a b\"}\n- one\n-   two\n:::\n\n::leaf[text]{k=v}\n::::\n\n```go\n:::not-a-directive\n```\n",
        "Para one\n\n\n\nPara   two\t\n\n> quote\n>   indented\n",
        "```markdown\n```\n```\n",
        "Intro\n\n```\n```\n\n~~~\n~~~\n\n- ```\n  ```\n\n```\n",
        " # A clear that cannot remove the override must not report the route as gone.\n+    if peer:\n",
        "   ## Deep\ntext\n\n Setext\n ===\n",
        "See [`peer path`](../peer/SKILL.md#x), [a `b`](y.md), [`c` d](z.md), [``e`f``](v.md) and [*`g` h*](w.md).\n",
        "| a | b |\n| - | - |\n| [x](u) | `y` |\n\n> 1. one\n>    two\n>\n> 2. three\n",
        "Setext\n===\n\n* a\n\n  b\n* [![alt](i.png)](l.md) <https://x.test>\n",
    ];
    for text in texts {
        assert_eq!(parse("doc.md", text).to_string(), text);
    }
}

#[test]
fn unedited_container_fences_are_written_verbatim() {
    for text in [
        "::::::a\nx\n::::::\n",
        ":::a\n:::b\nx\n:::\n:::\n",
        "::::a\nx\n:::::\n",
        ":::::a{k=1}\n:::b\n::::c\nx\n::::\n:::\n:::::\n",
    ] {
        assert_eq!(parse("fence.md", text).to_string(), text);
    }
}

#[test]
fn edited_container_fences_take_canonical_lengths() {
    let cases = [
        (":::a\nx\n::::::\n", "k", ":::a{k=1}\nx\n:::\n"),
        ("::::::a\nx\n::::::\n", "k", ":::a{k=1}\nx\n:::\n"),
        (
            ":::a\n:::b\nx\n:::\n:::\n",
            "k",
            "::::a{k=1}\n:::b\nx\n:::\n::::\n",
        ),
    ];
    for (text, key, want) in cases {
        let mut doc = parse("fence.md", text);
        find_mut(&mut doc, "directive", 0)
            .attrs
            .insert(key.to_owned(), "1".to_owned());
        let got = doc.to_string();
        assert_eq!(got, want, "{text:?}");
        assert_eq!(parse("fence.md", &got).to_string(), got);
    }
}

#[test]
fn a_container_outgrown_by_an_edited_child_lengthens_its_fence() {
    let mut doc = parse("fence.md", ":::a\n:::b\nx\n:::\n:::\n");
    let inner = find_mut(&mut doc, "directive", 0).children[0].clone();
    let mut grown = Item::new("directive", inner.origin.clone());
    grown.attrs.insert("name".to_owned(), "c".to_owned());
    grown.children.push(inner);
    find_mut(&mut doc, "directive", 0).children[0] = grown;
    let got = doc.to_string();
    assert_eq!(got, ":::::a\n::::c\n:::b\nx\n:::\n::::\n:::::\n");
    assert_eq!(parse("fence.md", &got).to_string(), got);
}

#[test]
fn mutated_trees_are_serialized() {
    check(vec![
        (
            "delete",
            "one\n\ntwo\n\nthree\n",
            Box::new(|doc| {
                doc.children.remove(1);
            }),
            "one\n\nthree\n",
        ),
        (
            "delete last",
            "one\n\ntwo\n",
            Box::new(|doc| doc.children.truncate(1)),
            "one\n",
        ),
        (
            "delete inside container",
            ":::box\nkeep\n\ndrop\n:::\n",
            Box::new(|doc| find_mut(doc, "directive", 0).children.truncate(1)),
            ":::box\nkeep\n:::\n",
        ),
        (
            "replace with graft",
            "one\n\ntwo\n\nthree\n",
            Box::new(|doc| {
                let other = parse("other.md", "intro\n\ngrafted  *text*\n");
                doc.children[1] = other.children[1].clone();
            }),
            "one\n\ngrafted  *text*\n\nthree\n",
        ),
        (
            "reorder",
            "one\n\ntwo\n\n\nthree\n",
            Box::new(|doc| reorder_in(&[], &[2, 0, 1]).apply(doc)),
            "three\n\none\n\ntwo\n",
        ),
        (
            "container attributes",
            ":::box{b=1 a=x}\nbody\n:::\n",
            Box::new(|doc| {
                let attrs = &mut find_mut(doc, "directive", 0).attrs;
                attrs.insert("a".to_owned(), "two words".to_owned());
                attrs.remove("b");
                attrs.insert("id".to_owned(), "top".to_owned());
            }),
            ":::box{#top a=\"two words\"}\nbody\n:::\n",
        ),
        (
            "leaf and inline attributes",
            "::leaf[text]{k=v}\n\nSee :ref[doc]{to=a} here.\n",
            Box::new(|doc| {
                find_mut(doc, "directive", 0)
                    .attrs
                    .insert("k".to_owned(), "w".to_owned());
                find_mut(doc, "directive", 1)
                    .attrs
                    .insert("to".to_owned(), "b".to_owned());
            }),
            "::leaf[text]{k=w}\n\nSee :ref[doc]{to=b} here.\n",
        ),
        (
            "sourceless items",
            "one\n",
            Box::new(|doc| {
                let mut note = item("directive", "");
                note.attrs.insert("name".to_owned(), "note".to_owned());
                note.attrs.insert("tone".to_owned(), "info".to_owned());
                let mut heading = item("heading", "Title");
                heading.attrs.insert("level".to_owned(), "2".to_owned());
                note.children = vec![heading, item("paragraph", "made")];
                let mut mark = item("directive", "");
                mark.attrs.insert("name".to_owned(), "mark".to_owned());
                doc.children.extend([note, mark]);
            }),
            "one\n\n:::note{tone=info}\n## Title\n\nmade\n:::\n\n::mark\n",
        ),
    ]);
}

#[test]
fn documents_built_outside_the_parser_are_written() {
    let mut inner = item("directive", "");
    inner.attrs.insert("name".to_owned(), "inner".to_owned());
    inner.children.push(item("paragraph", "x"));
    let mut outer = item("directive", "");
    outer.attrs.insert("name".to_owned(), "outer".to_owned());
    outer.children.push(inner);
    let mut doc = Document::new("doc.md");
    doc.children = vec![item("paragraph", "alpha"), outer];
    assert_eq!(
        doc.to_string(),
        "alpha\n\n::::outer\n:::inner\nx\n:::\n::::\n"
    );
}

#[test]
fn fences_inside_sections_take_canonical_lengths_once_edited() {
    let text = ":::a\n# H\n:::b\n:::c\nx\n:::\n:::\n:::\n";
    let mut doc = parse("doc.md", text);
    assert_eq!(doc.to_string(), text);
    find_mut(&mut doc, "directive", 0)
        .attrs
        .insert("k".to_owned(), "1".to_owned());
    let got = doc.to_string();
    assert_eq!(got, "::::a{k=1}\n# H\n:::b\n:::c\nx\n:::\n:::\n::::\n");
    assert_eq!(parse("doc.md", &got).to_string(), got);
}

const SYNTAX: &[(&str, &str, Edit, &str)] = &[
    (
        "bullet delete middle",
        "- a\n- b\n- c\n",
        without(&[0, 1]),
        "- a\n- c\n",
    ),
    (
        "bullet delete first",
        "- a\n- b\n- c\n",
        without(&[0, 0]),
        "- b\n- c\n",
    ),
    (
        "bullet reorder",
        "- a\n- b\n- c\n",
        reorder_list(&[2, 0, 1]),
        "- c\n- a\n- b\n",
    ),
    (
        "ordered delete middle",
        "1. a\n2. b\n3. c\n",
        without(&[0, 1]),
        "1. a\n3. c\n",
    ),
    (
        "ordered reorder",
        "1. a\n2. b\n3. c\n",
        reorder_list(&[2, 0, 1]),
        "3. c\n1. a\n2. b\n",
    ),
    (
        "loose list reorder",
        "- a\n\n- b\n",
        reorder_list(&[1, 0]),
        "- b\n\n- a\n",
    ),
    (
        "rule delete after",
        "A\n\n---\n\nB\n",
        without(&[2]),
        "A\n\n---\n",
    ),
    (
        "rule delete before",
        "A\n\n---\n\nB\n",
        without(&[0]),
        "---\n\nB\n",
    ),
    (
        "rule delete itself",
        "A\n\n---\n\nB\n",
        without(&[1]),
        "A\n\nB\n",
    ),
    (
        "quote delete middle",
        "> a\n>\n> b\n>\n> c\n",
        without(&[0, 1]),
        "> a\n>\n> c\n",
    ),
    (
        "quote delete first",
        "> a\n>\n> b\n",
        without(&[0, 0]),
        "> b\n",
    ),
    (
        "quote moved before paragraph",
        "x\n\n> q\n",
        reorder_in(&[], &[1, 0]),
        "> q\n\nx\n",
    ),
    (
        "list in quote delete first",
        "> - a\n> - b\n",
        without(&[0, 0, 0]),
        "> - b\n",
    ),
    (
        "fence in quote delete paragraph",
        "> a\n>\n> ```\n> x\n> ```\n",
        without(&[0, 0]),
        "> ```\n> x\n> ```\n",
    ),
    (
        "indented code delete paragraph",
        "a\n\n    code\n",
        without(&[0]),
        "    code\n",
    ),
    (
        "html delete neighbour",
        "a\n\n<div>\nx\n</div>\n",
        without(&[0]),
        "<div>\nx\n</div>\n",
    ),
    (
        "directive in item delete first",
        "- :::note\n  a\n\n  b\n  :::\n",
        without(&[0, 0, 0, 0]),
        "- :::note\n  b\n  :::\n",
    ),
    (
        "directive in item reorder",
        "- :::note\n  a\n\n  b\n  :::\n",
        reorder_in(&[0, 0, 0], &[1, 0]),
        "- :::note\n  b\n\n  a\n  :::\n",
    ),
    (
        "directive in item delete last",
        "- :::note\n  a\n\n  b\n  :::\n",
        without(&[0, 0, 0, 1]),
        "- :::note\n  a\n  :::\n",
    ),
    (
        "directive in quote delete first",
        "> :::note\n> a\n>\n> b\n> :::\n",
        without(&[0, 0, 0]),
        "> :::note\n> b\n> :::\n",
    ),
    (
        "directive in quote reorder",
        "> :::note\n> a\n>\n> b\n> :::\n",
        reorder_in(&[0, 0], &[1, 0]),
        "> :::note\n> b\n>\n> a\n> :::\n",
    ),
    (
        "directive in list in quote delete first",
        "> - :::note\n>   a\n>\n>   b\n>   :::\n",
        without(&[0, 0, 0, 0, 0]),
        "> - :::note\n>   b\n>   :::\n",
    ),
    (
        "directive in list in quote reorder",
        "> - :::note\n>   a\n>\n>   b\n>   :::\n",
        reorder_in(&[0, 0, 0, 0], &[1, 0]),
        "> - :::note\n>   b\n>\n>   a\n>   :::\n",
    ),
    (
        "directive in directive in item delete first",
        "- ::::outer\n  :::inner\n  a\n\n  b\n  :::\n  ::::\n",
        without(&[0, 0, 0, 0, 0]),
        "- ::::outer\n  :::inner\n  b\n  :::\n  ::::\n",
    ),
    (
        "list in directive in item delete first",
        "- :::note\n  - a\n  - b\n  :::\n",
        without(&[0, 0, 0, 0, 0]),
        "- :::note\n  - b\n  :::\n",
    ),
    (
        "quote in item delete first",
        "- > a\n  >\n  > b\n",
        without(&[0, 0, 0, 0]),
        "- > b\n",
    ),
    (
        "item delete first paragraph",
        "- a\n\n  b\n",
        without(&[0, 0, 0]),
        "- b\n",
    ),
    (
        "directive in item emptied",
        "- :::note\n  a\n  :::\n",
        without(&[0, 0, 0, 0]),
        "- :::note\n  :::\n",
    ),
    (
        "closed heading delete neighbour",
        "# a ##\n\nb\n",
        without(&[1]),
        "# a ##\n",
    ),
    (
        "directive in quote emptied",
        "> :::note\n> a\n> :::\n",
        without(&[0, 0, 0]),
        "> :::note\n> :::\n",
    ),
];

#[test]
fn items_own_their_syntax() {
    for &(name, text, edit, want) in SYNTAX {
        let mut doc = parse("doc.md", text);
        edit.apply(&mut doc);
        assert_eq!(doc.to_string(), want, "{name}");
    }
}

#[test]
fn generated_children_are_inserted() {
    let mut two = item("item", "");
    two.children = vec![item("paragraph", "b"), item("paragraph", "c")];
    check(vec![
        (
            "empty container",
            ":::note\n:::\n",
            append_to(&[0], item("paragraph", "new")),
            ":::note\nnew\n:::\n",
        ),
        (
            "empty container in item",
            "- :::note\n  :::\n",
            append_to(&[0, 0, 0], item("paragraph", "new")),
            "- :::note\n  new\n  :::\n",
        ),
        (
            "empty container in quote",
            "> :::note\n> :::\n",
            append_to(&[0, 0], item("paragraph", "new")),
            "> :::note\n> new\n> :::\n",
        ),
        (
            "multi-paragraph item in list in quote",
            "> - a\n",
            append_to(&[0, 0], two),
            "> - a\n> - b\n>\n>   c\n",
        ),
    ]);
}

#[test]
fn changed_setext_headings_keep_their_underline() {
    let retext: fn() -> Mutation =
        || Box::new(|doc| "Title X".clone_into(&mut doc.children[0].text));
    check(vec![
        (
            "level one",
            "Title :slot[x]\n=====\n\nbody\n",
            retext(),
            "Title X\n=====\n\nbody\n",
        ),
        (
            "level two",
            "Title :slot[x]\n-----\n",
            retext(),
            "Title X\n-----\n",
        ),
    ]);
}

#[test]
fn moved_html_keeps_its_closing_line() {
    for (text, want) in [
        (":::note\n<!--\nnote\n-->\n:::\n", "<!--\nnote\n-->\n"),
        (":::note\n<pre>\nx\n</pre>\n:::\n", "<pre>\nx\n</pre>\n"),
    ] {
        let mut doc = parse("doc.md", text);
        doc.children = doc.children[0].children.clone();
        assert_eq!(doc.to_string(), want);
    }
}

#[test]
fn inline_children_keep_their_text() {
    type Case = (&'static str, fn(&mut Item), &'static str);
    let cases: [Case; 4] = [
        (
            "See :a[] and *x* :b[] end.\n",
            |p| p.children = vec![item("text", "A"), item("text", "B")],
            "See A and *x* B end.\n",
        ),
        ("A :empty[] Z\n", |p| p.children.clear(), "A  Z\n"),
        (
            "A :a[] B :b[] C\n",
            |p| {
                p.children.remove(0);
            },
            "A  B :b[] C\n",
        ),
        ("A :a[]\nZ\n", |p| p.children.clear(), "A \nZ\n"),
    ];
    for (text, mutate, want) in cases {
        let mut doc = parse("doc.md", text);
        mutate(&mut doc.children[0]);
        assert_eq!(doc.to_string(), want, "{text:?}");
    }
}

#[test]
fn replacements_take_their_anchors_position() {
    let mut doc = parse("doc.md", "A :a[] B :b[] C\n");
    let paragraph = &mut doc.children[0];
    paragraph.children = paragraph.children[1].replace([item("text", "X")]);
    assert_eq!(doc.to_string(), "A  B X C\n");

    let mut doc = parse("doc.md", "a\n\nb\n\n\n\nc\n");
    let mut children = vec![doc.children[1].clone()];
    children.extend(doc.children[2].replace([item("paragraph", "Z")]));
    doc.children = children;
    assert_eq!(doc.to_string(), "b\n\n\n\nZ\n");
}

#[test]
fn rewriting_a_link_changes_only_its_destination() {
    let text = "# Links  \n\nSee [the *guide*](a/b.md \"Title\")  and\t![logo](<img/a b.png>)\nthen [ref][r], <https://x.test/a> and :term[[doc](d.md)]{k=v}.\n\n[r]: https://r.test\n";
    let cases = [
        ("link", 0, "c.md", text.replacen("a/b.md", "c.md", 1)),
        (
            "image",
            0,
            "img/c.png",
            text.replacen("<img/a b.png>", "<img/c.png>", 1),
        ),
        (
            "link",
            1,
            "new.md",
            text.replacen("[ref][r]", "[ref](new.md)", 1),
        ),
        (
            "link",
            2,
            "https://y.test",
            text.replacen("https://x.test/a", "https://y.test", 1),
        ),
        (
            "link",
            3,
            "e f.md",
            text.replacen("(d.md)", "(<e f.md>)", 1),
        ),
    ];
    for (kind, nth, url, want) in cases {
        let mut doc = parse("doc.md", text);
        assert_eq!(doc.to_string(), text);
        find_mut(&mut doc, kind, nth)
            .attrs
            .insert("url".to_owned(), url.to_owned());
        assert_eq!(doc.to_string(), want, "{kind} {nth}");
        let again = parse("doc.md", &want);
        assert_eq!(find(&again, kind, nth).attrs["url"], url);
    }
}

#[test]
fn edited_inline_items_take_canonical_form() {
    let mut doc = parse("doc.md", "See [a](u) and `b`.\n");
    "x y".clone_into(&mut find_mut(&mut doc, "link", 0).text);
    "c`d".clone_into(&mut find_mut(&mut doc, "code", 0).text);
    assert_eq!(doc.to_string(), "See [x y](u) and ``c`d``.\n");
}
