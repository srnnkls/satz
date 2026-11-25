//! YAML scalar resolution as henia's YAML library (`gopkg.in/yaml.v3`)
//! performs it: the short tag of a scalar and the value it decodes to.

/// A decoded scalar.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Timestamp,
    Str(String),
}

impl Value {
    /// The value as Go's `fmt.Sprint` prints it, for the kinds henia keeps:
    /// strings, booleans, `int`s and floats.
    pub(super) fn display(&self) -> Option<String> {
        match self {
            Self::Str(s) => Some(s.clone()),
            Self::Bool(b) => Some(b.to_string()),
            Self::Int(i) => Some(i.to_string()),
            Self::Float(f) => Some(go_float(*f)),
            Self::Null | Self::Uint(_) | Self::Timestamp => None,
        }
    }
}

const PREFIX: &str = "tag:yaml.org,2002:";

/// The short form of `tag`: `tag:yaml.org,2002:int` is `!!int`.
pub(super) fn short_tag(tag: &str) -> String {
    tag.strip_prefix(PREFIX)
        .map_or_else(|| tag.to_owned(), |suffix| format!("!!{suffix}"))
}

/// The tag yaml.v3 gives a scalar: its explicit tag, `!!str` when quoted or
/// a block, else the tag its plain value resolves to.
pub(super) fn scalar_tag(explicit: Option<&str>, plain: bool, value: &str) -> String {
    match explicit {
        Some(tag) if !tag.is_empty() && tag != "!" => short_tag(tag),
        _ if !plain => "!!str".to_owned(),
        _ if value == "<<" => "!!merge".to_owned(),
        _ => resolve(value).0.to_owned(),
    }
}

/// Decodes a scalar of short `tag`, as yaml.v3 decodes it into an `any`.
///
/// # Errors
/// Returns yaml.v3's message when the value does not decode as its tag.
pub(super) fn decode(tag: &str, value: &str) -> Result<Value, String> {
    match tag {
        "!!binary" => {
            return decode_base64(value)
                .map(|bytes| Value::Str(String::from_utf8_lossy(&bytes).into_owned()))
                .ok_or_else(|| "!!binary value contains invalid base64 data".to_owned());
        }
        "!!bool" | "!!null" | "!!int" | "!!float" | "!!timestamp" => {}
        _ => return Ok(Value::Str(value.to_owned())),
    }
    let (resolved, decoded) = resolve(value);
    match (tag, decoded) {
        (_, decoded) if resolved == tag => Ok(decoded),
        ("!!float", Value::Int(i)) => Ok(Value::Float(float_of(i))),
        _ => Err(format!("cannot decode {resolved} `{value}` as a {tag}")),
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "yaml.v3 converts an !!int tagged !!float with Go's float64(int)"
)]
fn float_of(i: i64) -> f64 {
    i as f64
}

/// yaml.v3's `resolve` for a plain scalar without a tag.
pub(super) fn resolve(value: &str) -> (&'static str, Value) {
    match value {
        "true" | "True" | "TRUE" => return ("!!bool", Value::Bool(true)),
        "false" | "False" | "FALSE" => return ("!!bool", Value::Bool(false)),
        "" | "~" | "null" | "Null" | "NULL" => return ("!!null", Value::Null),
        ".nan" | ".NaN" | ".NAN" => return ("!!float", Value::Float(f64::NAN)),
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => {
            return ("!!float", Value::Float(f64::INFINITY));
        }
        "-.inf" | "-.Inf" | "-.INF" => return ("!!float", Value::Float(f64::NEG_INFINITY)),
        "<<" => return ("!!merge", Value::Str(value.to_owned())),
        _ => {}
    }
    let string = || ("!!str", Value::Str(value.to_owned()));
    match value.as_bytes()[0] {
        b'.' => go_parse_float(value).map_or_else(string, |f| ("!!float", Value::Float(f))),
        b'0'..=b'9' | b'+' | b'-' => {
            if is_timestamp(value) {
                return ("!!timestamp", Value::Timestamp);
            }
            let plain = value.replace('_', "");
            if let Some(i) = go_parse_int(&plain) {
                return ("!!int", Value::Int(i));
            }
            if let Some(u) = go_parse_uint(&plain) {
                return ("!!int", Value::Uint(u));
            }
            if yaml_style_float(&plain)
                && let Some(f) = go_parse_float(&plain)
            {
                return ("!!float", Value::Float(f));
            }
            prefixed(&plain).map_or_else(string, |v| ("!!int", v))
        }
        _ => string(),
    }
}

/// yaml.v3's `0b` and `0o` integers, which Go's base prefixes would read
/// with a sign or underscores yaml.v3 strips first.
fn prefixed(plain: &str) -> Option<Value> {
    let (negative, rest) = plain
        .strip_prefix('-')
        .map_or((false, plain), |rest| (true, rest));
    let (radix, digits) = rest
        .strip_prefix("0b")
        .map(|digits| (2, digits))
        .or_else(|| rest.strip_prefix("0o").map(|digits| (8, digits)))?;
    if digits.starts_with(['+', '-']) {
        return None;
    }
    if negative {
        let digits = format!("-{digits}");
        return i64::from_str_radix(&digits, radix).ok().map(Value::Int);
    }
    i64::from_str_radix(digits, radix)
        .map(Value::Int)
        .ok()
        .or_else(|| u64::from_str_radix(digits, radix).ok().map(Value::Uint))
}

/// Go's `strconv.ParseInt(s, 0, 64)` on a string without underscores.
fn go_parse_int(s: &str) -> Option<i64> {
    let (negative, rest) = match s.as_bytes().first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    let magnitude = go_parse_uint(rest)?;
    if negative {
        0i64.checked_sub_unsigned(magnitude)
    } else {
        i64::try_from(magnitude).ok()
    }
}

/// Go's `strconv.ParseUint(s, 0, 64)` on a string without underscores.
fn go_parse_uint(s: &str) -> Option<u64> {
    let lower = s.get(..2).map(str::to_ascii_lowercase);
    let (radix, digits) = match lower.as_deref() {
        Some("0x") => (16, &s[2..]),
        Some("0o") => (8, &s[2..]),
        Some("0b") => (2, &s[2..]),
        _ if s.len() > 1 && s.starts_with('0') => (8, &s[1..]),
        _ => (10, s),
    };
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    u64::from_str_radix(digits, radix).ok()
}

/// Go's `strconv.ParseFloat(s, 64)` for the decimal forms YAML scalars take;
/// out-of-range values are errors, as in Go.
fn go_parse_float(s: &str) -> Option<f64> {
    let digits = s.trim_start_matches(['+', '-']);
    if !digits
        .bytes()
        .all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return None;
    }
    s.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// `^[-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?$`
fn yaml_style_float(s: &str) -> bool {
    let s = s.strip_prefix(['-', '+']).unwrap_or(s);
    let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
    let mantissa = if let Some(fraction) = s.strip_prefix('.') {
        match digits(fraction) {
            0 => return false,
            n => 1 + n,
        }
    } else {
        let int = digits(s);
        if int == 0 {
            return false;
        }
        match s[int..].strip_prefix('.') {
            Some(fraction) => int + 1 + digits(fraction),
            None => int,
        }
    };
    let rest = &s[mantissa..];
    if rest.is_empty() {
        return true;
    }
    let Some(exponent) = rest.strip_prefix(['e', 'E']) else {
        return false;
    };
    let exponent = exponent.strip_prefix(['-', '+']).unwrap_or(exponent);
    !exponent.is_empty() && digits(exponent) == exponent.len()
}

/// yaml.v3's timestamps: `YYYY-M-D`, optionally followed by a time after
/// `T`, `t` or a space, with a zone only after `T` or `t`.
fn is_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 5 || !b[..4].iter().all(u8::is_ascii_digit) || b[4] != b'-' {
        return false;
    }
    let mut at = 5;
    let (Some(month), Some(day)) = (
        number(b, &mut at).filter(|m| (1..=12).contains(m)),
        expect(b, &mut at, b'-')
            .then(|| number(b, &mut at))
            .flatten(),
    ) else {
        return false;
    };
    let year: u32 = s[..4].parse().unwrap_or_default();
    if day == 0 || day > days_in(month, year) {
        return false;
    }
    match b.get(at) {
        None => true,
        Some(&sep @ (b'T' | b't' | b' ')) => {
            at += 1;
            if !time(b, &mut at) {
                return false;
            }
            if sep == b' ' {
                return at == b.len();
            }
            zone(b, &mut at) && at == b.len()
        }
        Some(_) => false,
    }
}

/// One or two digits, as Go's non-padded layout fields read them.
fn number(b: &[u8], at: &mut usize) -> Option<u32> {
    let n = b[*at..]
        .iter()
        .take(2)
        .take_while(|c| c.is_ascii_digit())
        .count();
    if n == 0 {
        return None;
    }
    let value = b[*at..*at + n]
        .iter()
        .fold(0, |acc, &c| acc * 10 + u32::from(c - b'0'));
    *at += n;
    Some(value)
}

fn expect(b: &[u8], at: &mut usize, c: u8) -> bool {
    let found = b.get(*at) == Some(&c);
    if found {
        *at += 1;
    }
    found
}

fn time(b: &[u8], at: &mut usize) -> bool {
    let hour = number(b, at).is_some_and(|h| h < 24);
    let minute = hour && expect(b, at, b':') && number(b, at).is_some_and(|m| m < 60);
    let second = minute && expect(b, at, b':') && number(b, at).is_some_and(|s| s < 60);
    if !second {
        return false;
    }
    if matches!(b.get(*at), Some(b'.' | b',')) && b.get(*at + 1).is_some_and(u8::is_ascii_digit) {
        let digits = b[*at + 1..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digits > 9 {
            return false;
        }
        *at += 1 + digits;
    }
    true
}

fn zone(b: &[u8], at: &mut usize) -> bool {
    match b.get(*at) {
        Some(b'Z') => {
            *at += 1;
            true
        }
        Some(b'+' | b'-') => {
            let fixed = |s: &[u8]| s.len() == 2 && s.iter().all(u8::is_ascii_digit);
            let ok = b.len() >= *at + 6
                && fixed(&b[*at + 1..*at + 3])
                && b[*at + 3] == b':'
                && fixed(&b[*at + 4..*at + 6]);
            *at += 6;
            ok
        }
        _ => false,
    }
}

fn days_in(month: u32, year: u32) -> u32 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Go's `%v` of a `float64`: the shortest digits, in exponent form below
/// 1e-4 and from 1e6 on.
fn go_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_owned();
    }
    if f.is_infinite() {
        return if f > 0.0 { "+Inf" } else { "-Inf" }.to_owned();
    }
    let scientific = format!("{f:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or_default();
    if !(-4..6).contains(&exponent) {
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{mantissa}e{sign}{:02}", exponent.unsigned_abs());
    }
    f.to_string()
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let clean: Vec<u8> = text
        .bytes()
        .filter(|c| !matches!(c, b'\r' | b'\n'))
        .collect();
    if !clean.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for (i, chunk) in clean.chunks(4).enumerate() {
        let last = i + 1 == clean.len() / 4;
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut bits = 0u32;
        for &c in &chunk[..4 - pad] {
            bits = bits << 6 | u32::from(value(c)?);
        }
        bits <<= 6 * pad;
        let bytes = bits.to_be_bytes();
        out.extend_from_slice(&bytes[1..4 - pad]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(value: &str) -> &'static str {
        resolve(value).0
    }

    #[test]
    fn plain_scalars_resolve_as_yaml_v3_resolves_them() {
        for (value, expected) in [
            ("tags", "!!str"),
            ("12", "!!int"),
            ("0x1F", "!!int"),
            ("0o17", "!!int"),
            ("0b101", "!!int"),
            ("-0b101", "!!int"),
            ("0777", "!!int"),
            ("1_000", "!!int"),
            ("+12", "!!int"),
            ("08", "!!float"),
            ("18446744073709551615", "!!int"),
            ("0.5", "!!float"),
            (".5", "!!float"),
            ("1e3", "!!float"),
            ("1.", "!!float"),
            (".inf", "!!float"),
            ("true", "!!bool"),
            ("yes", "!!str"),
            ("on", "!!str"),
            ("null", "!!null"),
            ("~", "!!null"),
            ("2026-11-25", "!!timestamp"),
            ("2026-1-5", "!!timestamp"),
            ("2026-11-25T10:20:30Z", "!!timestamp"),
            ("2026-11-25t1:2:3.5+01:00", "!!timestamp"),
            ("2026-11-25 10:20:30", "!!timestamp"),
            ("2026-11-25 10:20:30Z", "!!str"),
            ("2026-02-30", "!!str"),
            ("2026-13-01", "!!str"),
            ("1.2.3", "!!str"),
            ("-", "!!str"),
            ("1e", "!!str"),
            ("1e400", "!!str"),
            ("0x", "!!str"),
            ("0_1", "!!int"),
        ] {
            assert_eq!(tag(value), expected, "{value}");
        }
    }

    #[test]
    fn tags_follow_style_and_explicit_tags() {
        assert_eq!(scalar_tag(None, false, "123"), "!!str");
        assert_eq!(
            scalar_tag(Some("tag:yaml.org,2002:int"), true, "1"),
            "!!int"
        );
        assert_eq!(scalar_tag(Some("!custom"), true, "x"), "!custom");
        assert_eq!(scalar_tag(Some("!"), true, "12"), "!!int");
        assert_eq!(scalar_tag(None, true, "<<"), "!!merge");
    }

    #[test]
    fn decoding_checks_explicit_tags() {
        assert_eq!(decode("!!str", "12"), Ok(Value::Str("12".to_owned())));
        assert_eq!(decode("!!float", "1"), Ok(Value::Float(1.0)));
        assert_eq!(decode("!!binary", "aGk="), Ok(Value::Str("hi".to_owned())));
        assert_eq!(
            decode("!!int", "abc"),
            Err("cannot decode !!str `abc` as a !!int".to_owned())
        );
        assert_eq!(decode("!custom", "x"), Ok(Value::Str("x".to_owned())));
    }

    #[test]
    fn values_print_as_go_prints_them() {
        let shown = |value: &str| resolve(value).1.display();
        assert_eq!(shown("0.5").as_deref(), Some("0.5"));
        assert_eq!(shown("1e3").as_deref(), Some("1000"));
        assert_eq!(shown("1e21").as_deref(), Some("1e+21"));
        assert_eq!(shown("1234567.0").as_deref(), Some("1.234567e+06"));
        assert_eq!(shown("100000.0").as_deref(), Some("100000"));
        assert_eq!(shown("08").as_deref(), Some("8"));
        assert_eq!(shown("0.00001").as_deref(), Some("1e-05"));
        assert_eq!(shown("0x1F").as_deref(), Some("31"));
        assert_eq!(shown(".inf").as_deref(), Some("+Inf"));
        assert_eq!(shown("2026-11-25"), None);
        assert_eq!(shown("null"), None);
    }
}
