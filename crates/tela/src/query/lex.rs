//! The tq lexer: byte-offset tokens over a query or module source.

use std::collections::BTreeMap;

use super::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Eof,
    Open,
    Close,
    OpenAlt,
    CloseAlt,
    Direct,
    Anchor,
    Quant,
    Capture,
    Key,
    String,
    Int,
    Range,
    Symbol,
    Regex,
    Var,
    Float,
    Param,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Token {
    pub(super) kind: Kind,
    pub(super) text: String,
    pub(super) pos: usize,
    pub(super) lo: f64,
    pub(super) hi: f64,
    /// For a regexp a define expands: each named group's binding, `?var`
    /// or `=value`.
    pub(super) groups: BTreeMap<String, String>,
}

impl Token {
    fn new(kind: Kind, text: impl Into<String>, pos: usize) -> Self {
        Self {
            kind,
            text: text.into(),
            pos,
            lo: 0.0,
            hi: 0.0,
            groups: BTreeMap::new(),
        }
    }

    pub(super) fn eof(pos: usize) -> Self {
        Self::new(Kind::Eof, "", pos)
    }

    pub(super) fn is_symbol(&self, text: &str) -> bool {
        self.kind == Kind::Symbol && self.text == text
    }

    /// The token as a define signature names it: `@name` or `?name`.
    pub(super) fn sigil(&self) -> String {
        let sigil = if self.kind == Kind::Capture { '@' } else { '?' };
        format!("{sigil}{}", self.text)
    }
}

pub(super) fn is_name(c: u8, first: bool) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || !first && (c.is_ascii_digit() || c == b'-' || c == b'.')
}

fn is_symbol(c: u8, first: bool) -> bool {
    is_name(c, first) || !first && c == b'/'
}

/// The end of the name starting at `start`.
fn name_end(src: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < src.len() && is_name(src[end], end == start) {
        end += 1;
    }
    end
}

fn digits_end(src: &[u8], start: usize) -> usize {
    start
        + src[start..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
}

fn number(digits: &str) -> f64 {
    digits.parse().unwrap_or(f64::MAX)
}

/// Splits `source` into tokens ending in [`Kind::Eof`].
pub(super) fn lex(source: &str) -> Result<Vec<Token>, Error> {
    let src = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        let next = src.get(i + 1).copied();
        match c {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b';' => i += src[i..].iter().take_while(|&&c| c != b'\n').count(),
            b'$' | b'?' if next.is_some_and(|n| is_name(n, true)) => {
                let end = name_end(src, i + 1);
                let kind = if c == b'$' { Kind::Param } else { Kind::Var };
                tokens.push(Token::new(kind, &source[i + 1..end], i));
                i = end;
            }
            b'<' | b'>' if c == b'<' || next == Some(b'=') => {
                let end = if next == Some(b'=') { i + 2 } else { i + 1 };
                tokens.push(Token::new(Kind::Symbol, &source[i..end], i));
                i = end;
            }
            b'(' | b')' | b'[' | b']' | b'>' | b'?' | b'*' | b'+' => {
                let kind = match c {
                    b'(' => Kind::Open,
                    b')' => Kind::Close,
                    b'[' => Kind::OpenAlt,
                    b']' => Kind::CloseAlt,
                    b'>' => Kind::Direct,
                    _ => Kind::Quant,
                };
                tokens.push(Token::new(kind, &source[i..=i], i));
                i += 1;
            }
            b'.' if next == Some(b'.') && src.get(i + 2).is_some_and(u8::is_ascii_digit) => {
                let end = digits_end(src, i + 2);
                let mut token = Token::new(Kind::Range, &source[i..end], i);
                token.hi = number(&source[i + 2..end]);
                tokens.push(token);
                i = end;
            }
            b'.' => {
                tokens.push(Token::new(Kind::Anchor, ".", i));
                i += 1;
            }
            b'@' | b':' => {
                let end = name_end(src, i + 1);
                if end == i + 1 {
                    let what = if c == b'@' { "a capture name" } else { "a key" };
                    return Err(Error::syntax(
                        i,
                        format!("expected {what} after {:?}", char::from(c)),
                    ));
                }
                let kind = if c == b'@' { Kind::Capture } else { Kind::Key };
                tokens.push(Token::new(kind, &source[i + 1..end], i));
                i = end;
            }
            b'/' => {
                let mut j = i + 1;
                while j < src.len() && src[j] != b'/' {
                    j += if src[j] == b'\\' { 2 } else { 1 };
                }
                if j >= src.len() {
                    return Err(Error::syntax(i, "unterminated /regexp/")
                        .with_hint(r#"quote values that contain /, as in :path "ops/run.md""#));
                }
                let text = source[i + 1..j].replace(r"\/", "/");
                tokens.push(Token::new(Kind::Regex, text, i));
                i = j + 1;
            }
            b'"' => {
                let (text, end) = string(source, i)?;
                tokens.push(Token::new(Kind::String, text, i));
                i = end;
            }
            b'0'..=b'9' => i = lex_number(source, i, &mut tokens),
            c if is_name(c, true) => {
                let mut end = i;
                while end < src.len() && is_symbol(src[end], end == i) {
                    end += 1;
                }
                tokens.push(Token::new(Kind::Symbol, &source[i..end], i));
                i = end;
            }
            _ => {
                let found = source[i..]
                    .chars()
                    .next()
                    .unwrap_or(char::REPLACEMENT_CHARACTER);
                let hint = if found == '\'' {
                    "quote strings with \" inside the single-quoted shell argument"
                } else {
                    "patterns use ( ) [ ] > . ? * + @capture :key \"string\" /regexp/"
                };
                return Err(Error::syntax(i, format!("unexpected {found:?}")).with_hint(hint));
            }
        }
    }
    tokens.push(Token::eof(src.len()));
    Ok(tokens)
}

/// The string starting with the quote at `start`, and the offset after its
/// closing quote.
fn string(source: &str, start: usize) -> Result<(String, usize), Error> {
    let mut text = String::new();
    let mut chars = source[start + 1..].char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => return Ok((text, start + 1 + at + 1)),
            '\\' => match chars.next() {
                Some((_, 'n')) => text.push('\n'),
                Some((_, 't')) => text.push('\t'),
                Some((_, escaped)) => text.push(escaped),
                None => text.push('\\'),
            },
            c => text.push(c),
        }
    }
    Err(Error::syntax(start, "unterminated string"))
}

/// Lexes the integer, float or range at `start`, returning the offset after
/// it.
fn lex_number(source: &str, start: usize, tokens: &mut Vec<Token>) -> usize {
    let src = source.as_bytes();
    let end = digits_end(src, start);
    let lo = number(&source[start..end]);
    if src[end..].starts_with(b"..") {
        let range_end = digits_end(src, end + 2);
        let mut token = Token::new(Kind::Range, &source[start..range_end], start);
        token.lo = lo;
        token.hi = if range_end > end + 2 {
            number(&source[end + 2..range_end])
        } else {
            f64::INFINITY
        };
        tokens.push(token);
        return range_end;
    }
    if src.get(end) == Some(&b'.') && src.get(end + 1).is_some_and(u8::is_ascii_digit) {
        let float_end = digits_end(src, end + 1);
        tokens.push(Token::new(Kind::Float, &source[start..float_end], start));
        return float_end;
    }
    let mut token = Token::new(Kind::Int, &source[start..end], start);
    token.lo = lo;
    token.hi = lo;
    tokens.push(token);
    end
}
