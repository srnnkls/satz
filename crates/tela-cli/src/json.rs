//! JSON as henia's Go encoder writes it: two-space indent, numbers in Go's
//! shortest form (`8`, `0.5`, `1e-7`, `1e+21`) and U+2028 and U+2029
//! escaped. `<`, `>` and `&` stay as they are (AD-25).

use std::io::{self, Write};

use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter, Serializer};

/// Writes `value` and a newline.
pub(crate) fn write(out: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    let mut serializer = Serializer::with_formatter(&mut *out, Go(PrettyFormatter::new()));
    value.serialize(&mut serializer).map_err(io::Error::from)?;
    out.write_all(b"\n")
}

struct Go<'a>(PrettyFormatter<'a>);

impl Formatter for Go<'_> {
    fn write_f64<W: ?Sized + Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(go_float(value).as_bytes())
    }

    fn write_f32<W: ?Sized + Write>(&mut self, writer: &mut W, value: f32) -> io::Result<()> {
        self.write_f64(writer, f64::from(value))
    }

    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut rest = fragment;
        while let Some(index) = rest.find(['\u{2028}', '\u{2029}']) {
            writer.write_all(&rest.as_bytes()[..index])?;
            let separator = if rest[index..].starts_with('\u{2028}') {
                b"\\u2028"
            } else {
                b"\\u2029"
            };
            writer.write_all(separator)?;
            rest = &rest[index + '\u{2028}'.len_utf8()..];
        }
        writer.write_all(rest.as_bytes())
    }

    fn begin_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_array(writer)
    }

    fn end_array<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_array_value(writer)
    }

    fn begin_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object(writer)
    }

    fn end_object<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_object_key(writer, first)
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.0.end_object_value(writer)
    }
}

/// `value` as Go's `encoding/json` writes a float64: positional unless it is
/// below 1e-6 or from 1e21 up, shortest round-trip digits either way.
fn go_float(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude != 0.0 && !(1e-6..1e21).contains(&magnitude) {
        let scientific = format!("{value:e}");
        return match scientific.split_once('e') {
            Some((mantissa, exponent)) if !exponent.starts_with('-') => {
                format!("{mantissa}e+{exponent}")
            }
            _ => scientific,
        };
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(value: &impl Serialize) -> String {
        let mut out = Vec::new();
        write(&mut out, value).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn floats_follow_go() {
        let cases = [
            (8.0, "8"),
            (0.5, "0.5"),
            (0.428_571_428_571_428_55, "0.42857142857142855"),
            (1e-7, "1e-7"),
            (1.5e21, "1.5e+21"),
            (-0.0, "-0"),
        ];
        for (value, expected) in cases {
            assert_eq!(go_float(value), expected);
        }
    }

    #[test]
    fn line_separators_are_escaped_and_html_is_not() {
        assert_eq!(
            encode(&"a\u{2028}b\u{2029}<&>"),
            "\"a\\u2028b\\u2029<&>\"\n"
        );
    }

    #[test]
    fn indents_by_two_spaces() {
        let value = serde_json::json!({"b": [1.0, 2.5], "a": {}, "c": []});
        assert_eq!(
            encode(&value),
            "{\n  \"a\": {},\n  \"b\": [\n    1,\n    2.5\n  ],\n  \"c\": []\n}\n"
        );
    }
}
