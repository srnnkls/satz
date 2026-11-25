//! Directive attribute lists: `{#id .class key=value key="quoted" flag}`.

/// Attributes in source order; repeated classes are joined.
pub(crate) type Attrs = Vec<(String, String)>;

/// Malformed directive syntax at a byte offset of the text it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fault {
    pub(crate) offset: usize,
    pub(crate) id: Option<&'static str>,
    pub(crate) message: String,
}

impl Fault {
    pub(crate) fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            id: None,
            message: message.into(),
        }
    }

    pub(crate) fn with_id(offset: usize, id: &'static str, message: impl Into<String>) -> Self {
        Self {
            id: Some(id),
            ..Self::new(offset, message)
        }
    }

    /// The fault moved `by` bytes further into the text.
    pub(crate) fn shifted(self, by: usize) -> Self {
        Self {
            offset: self.offset + by,
            ..self
        }
    }
}

/// A closed attribute list: its attributes, its length and the repeated
/// attributes it reported.
#[derive(Debug, Default)]
pub(crate) struct Tokens {
    pub(crate) attrs: Attrs,
    pub(crate) consumed: usize,
    pub(crate) faults: Vec<Fault>,
}

/// Reads the attribute list `input` opens with. An unreadable list returns
/// every fault met, the fatal one last.
pub(crate) fn tokenize(input: &[u8]) -> Result<Tokens, Vec<Fault>> {
    let mut tokens = Tokens::default();
    let invalid = |mut faults: Vec<Fault>, offset, message: String| {
        faults.push(Fault::new(offset, message));
        Err(faults)
    };
    if input.first() != Some(&b'{') {
        return invalid(tokens.faults, 0, "invalid directive attributes".to_owned());
    }
    let mut i = 1;
    loop {
        i = skip_blanks(input, i);
        let Some(&byte) = input.get(i).filter(|&&b| b != b'\n' && b != b'\r') else {
            return invalid(tokens.faults, i, "unclosed directive attributes".to_owned());
        };
        let at = i;
        match byte {
            b'}' => {
                tokens.consumed = i + 1;
                return Ok(tokens);
            }
            b'#' | b'.' => {
                let len = bare_length(&input[i + 1..]);
                if len == 0 {
                    let message = format!("missing {} value", char::from(byte));
                    return invalid(tokens.faults, i, message);
                }
                let name = if byte == b'.' { "class" } else { "id" };
                tokens.add(at, name.to_owned(), lossy(&input[i + 1..=i + len]));
                i += 1 + len;
            }
            _ => {
                let len = name_length(&input[i..]);
                if len == 0 {
                    return invalid(tokens.faults, i, "invalid attribute name".to_owned());
                }
                let name = lossy(&input[i..i + len]);
                i += len;
                let mut value = "true".to_owned();
                let equals = skip_blanks(input, i);
                if input.get(equals) == Some(&b'=') {
                    let start = skip_blanks(input, equals + 1);
                    match attribute_value(&input[start..]) {
                        Ok((decoded, length)) => {
                            value = decoded;
                            i = start + length;
                        }
                        Err(error) => {
                            let message = format!("attribute {name:?}: {error}");
                            return invalid(tokens.faults, start, message);
                        }
                    }
                }
                if !valid_value(&value) {
                    return invalid(
                        tokens.faults,
                        at,
                        format!("attribute {name:?} contains an invalid XML character"),
                    );
                }
                tokens.add(at, name, value);
            }
        }
        if let Some(&byte) = input.get(i).filter(|&&b| !matches!(b, b'}' | b' ' | b'\t')) {
            return invalid(
                tokens.faults,
                i,
                format!("unexpected {} in directive attributes", quote_byte(byte)),
            );
        }
    }
}

impl Tokens {
    fn add(&mut self, at: usize, name: String, value: String) {
        match self.attrs.iter_mut().find(|(n, _)| *n == name) {
            Some((_, existing)) if name == "class" => {
                existing.push(' ');
                existing.push_str(&value);
            }
            Some(_) => self.faults.push(Fault::with_id(
                at,
                "duplicate-attribute",
                format!("attribute {name:?} is repeated"),
            )),
            None => self.attrs.push((name, value)),
        }
    }
}

/// The length of the directive or attribute name `s` opens with.
pub(crate) fn name_length(s: &[u8]) -> usize {
    s.iter()
        .enumerate()
        .position(|(i, &c)| {
            !(c.is_ascii_alphabetic()
                || c == b'_'
                || i > 0 && (c.is_ascii_digit() || c == b'-' || c == b'.'))
        })
        .unwrap_or(s.len())
}

/// The offset of the first byte at or after `i` that is not a space or tab.
pub(crate) fn skip_blanks(s: &[u8], i: usize) -> usize {
    s.get(i..)
        .and_then(|rest| rest.iter().position(|&c| c != b' ' && c != b'\t'))
        .map_or(s.len().max(i), |n| i + n)
}

/// The length of the unquoted value `s` opens with; multi-byte characters
/// never end it.
pub(crate) fn bare_length(s: &[u8]) -> usize {
    s.iter()
        .position(|&c| c <= b' ' || b"\"'=<>`{}[]".contains(&c))
        .unwrap_or(s.len())
}

fn attribute_value(input: &[u8]) -> Result<(String, usize), &'static str> {
    match input.first() {
        None => Err("missing value"),
        Some(b'"') => {
            let mut i = 1;
            while let Some(&c) = input.get(i).filter(|&&c| c != b'\n') {
                match c {
                    b'\\' => i += 1,
                    b'"' => {
                        return json_string(&input[1..i])
                            .map(|value| (value, i + 1))
                            .ok_or("invalid quoted value");
                    }
                    _ => {}
                }
                i += 1;
            }
            Err("unclosed quoted value")
        }
        Some(b'\'') => match input[1..].iter().position(|&c| c == b'\'' || c == b'\n') {
            Some(end) if input[1 + end] == b'\'' => Ok((lossy(&input[1..=end]), end + 2)),
            _ => Err("unclosed quoted value"),
        },
        Some(_) => match bare_length(input) {
            0 => Err("missing value"),
            n => Ok((lossy(&input[..n]), n)),
        },
    }
}

/// Decodes the body of a JSON string literal as Go's `encoding/json` does:
/// lone surrogates become U+FFFD, raw control characters are rejected.
fn json_string(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                c @ ('"' | '\\' | '/') => out.push(c),
                'b' => out.push('\u{8}'),
                'f' => out.push('\u{c}'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => {
                    let high = hex4(&mut chars)?;
                    if (0xd800..0xdc00).contains(&high) {
                        let rest = chars.as_str();
                        let low = rest
                            .strip_prefix("\\u")
                            .and_then(|r| r.get(..4))
                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                            .filter(|low| (0xdc00..0xe000).contains(low));
                        if let Some(low) = low {
                            chars = rest[6..].chars();
                            let combined = 0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00);
                            out.push(char::from_u32(combined).unwrap_or('\u{fffd}'));
                            continue;
                        }
                    }
                    out.push(char::from_u32(high).unwrap_or('\u{fffd}'));
                }
                _ => return None,
            },
            c if c < ' ' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

fn hex4(chars: &mut std::str::Chars<'_>) -> Option<u32> {
    let digits = chars.as_str().get(..4)?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    if digits.starts_with('+') {
        return None;
    }
    *chars = chars.as_str()[4..].chars();
    Some(value)
}

fn valid_value(value: &str) -> bool {
    !value.chars().any(|r| {
        (r < '\u{20}' && !matches!(r, '\n' | '\r' | '\t')) || matches!(r, '\u{fffe}' | '\u{ffff}')
    })
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A byte quoted as Go's `%q` quotes it, as a rune literal.
fn quote_byte(b: u8) -> String {
    let c = char::from(b);
    let inner = match c {
        '\u{7}' => "\\a".to_owned(),
        '\u{8}' => "\\b".to_owned(),
        '\u{c}' => "\\f".to_owned(),
        '\n' => "\\n".to_owned(),
        '\r' => "\\r".to_owned(),
        '\t' => "\\t".to_owned(),
        '\u{b}' => "\\v".to_owned(),
        '\\' => "\\\\".to_owned(),
        '\'' => "\\'".to_owned(),
        c if c < ' ' || c == '\u{7f}' => format!("\\x{b:02x}"),
        c if ('\u{80}'..'\u{a0}').contains(&c) || c == '\u{ad}' => format!("\\u{b:04x}"),
        c => c.to_string(),
    };
    format!("'{inner}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(input: &str) -> Attrs {
        tokenize(input.as_bytes()).unwrap().attrs
    }

    fn fatal(input: &str) -> String {
        tokenize(input.as_bytes())
            .unwrap_err()
            .pop()
            .unwrap()
            .message
    }

    #[test]
    fn reads_every_attribute_form() {
        assert_eq!(
            attrs(r#"{#main .one .two k="a \"b\"" q='c' flag x=1}"#),
            [
                ("id", "main"),
                ("class", "one two"),
                ("k", "a \"b\""),
                ("q", "c"),
                ("flag", "true"),
                ("x", "1"),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
        );
        assert_eq!(attrs("{class=a .b}")[0].1, "a b");
        assert_eq!(attrs("{.a class=b}")[0].1, "a b");
    }

    #[test]
    fn decodes_quoted_values_as_json() {
        assert_eq!(attrs(r#"{a="line\nnext\ttab"}"#)[0].1, "line\nnext\ttab");
        assert_eq!(attrs(r#"{a="😀 \ud800"}"#)[0].1, "\u{1f600} \u{fffd}");
        assert_eq!(
            fatal(r#"{a="\q"}"#),
            "attribute \"a\": invalid quoted value"
        );
    }

    #[test]
    fn reports_malformed_lists() {
        assert_eq!(fatal("{a=}"), "attribute \"a\": missing value");
        assert_eq!(fatal("{a=\"x}"), "attribute \"a\": unclosed quoted value");
        assert_eq!(fatal("{=x}"), "invalid attribute name");
        assert_eq!(fatal("{a"), "unclosed directive attributes");
        assert_eq!(fatal("{a=[1,2]}"), "attribute \"a\": missing value");
        assert_eq!(
            fatal("{a=\"x\"y}"),
            "unexpected 'y' in directive attributes"
        );
        assert_eq!(
            fatal(r#"{a="\b"}"#),
            "attribute \"a\" contains an invalid XML character"
        );
        assert_eq!(fatal("{#}"), "missing # value");
    }

    #[test]
    fn repeated_attributes_are_reported_and_kept_once() {
        let tokens = tokenize(b"{k=1 k=2}").unwrap();
        assert_eq!(tokens.attrs, [("k".to_owned(), "1".to_owned())]);
        assert_eq!(
            tokens.faults,
            [Fault::with_id(
                5,
                "duplicate-attribute",
                "attribute \"k\" is repeated"
            )]
        );
        assert_eq!(tokens.consumed, 9);
    }
}
