//! Python `str` and number semantics on `&str` / `i64` / `f64`.
//!
//! Every index is a CHAR index (source text is Latin-1 decoded: one byte, one `char`), never a
//! byte index. Python methods that Rust's `str` already implements identically (`startswith`,
//! `endswith`, `in`, `join`, `str(int)`) are not wrapped: call the std method.
//!
//! Unicode caveat: predicates and `repr` are exact for chars up to U+00FF (verified against
//! Python for all 256) and approximate above that, which source text never contains.

use std::fmt::Write as _;

// ---------------------------------------------------------------- indexing and slicing

/// `len(s)`.
pub fn len(s: &str) -> usize {
    s.chars().count()
}

/// Byte offset of char index `idx` (clamped to the end).
fn byte_offset(s: &str, idx: usize) -> usize {
    if s.is_ascii() {
        idx.min(s.len())
    } else {
        s.char_indices().nth(idx).map_or(s.len(), |(b, _)| b)
    }
}

/// Clamp a Python slice bound into `0..=n`.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "bounded by n"
)]
fn clamp_bound(n: usize, i: i64) -> usize {
    let n = n as i64;
    (if i < 0 { (i + n).max(0) } else { i.min(n) }) as usize
}

/// `s[i]` (negative `i` counts from the end); `None` where Python raises `IndexError`.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "i is checked non-negative or negated"
)]
pub fn char_at(s: &str, i: i64) -> Option<char> {
    if i >= 0 {
        s.chars().nth(i as usize)
    } else {
        s.chars().rev().nth((-(i + 1)) as usize)
    }
}

/// `s[start:end]` with Python clamping and negative indices.
#[allow(clippy::cast_possible_wrap, reason = "string lengths fit in i64")]
pub fn slice(s: &str, start: Option<i64>, end: Option<i64>) -> &str {
    let n = len(s);
    let a = start.map_or(0, |i| clamp_bound(n, i));
    let b = end.map_or(n, |i| clamp_bound(n, i));
    if a >= b {
        ""
    } else {
        &s[byte_offset(s, a)..byte_offset(s, b)]
    }
}

// ---------------------------------------------------------------- character classes

/// `str.isspace` for one char (includes U+001C..U+001F, U+0085, U+00A0).
pub fn is_space_char(c: char) -> bool {
    matches!(c, '\t'..='\r' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// `str.isdigit` for one char (ASCII digits and the superscripts `²³¹` in Latin-1).
fn is_digit_char(c: char) -> bool {
    c.is_ascii_digit()
        || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}')
        || (c > '\u{ff}' && c.is_numeric())
}

/// `str.isnumeric` for one char.
fn is_numeric_char(c: char) -> bool {
    c.is_numeric()
}

fn is_alpha_char(c: char) -> bool {
    c.is_alphabetic()
}

fn all_nonempty(s: &str, f: fn(char) -> bool) -> bool {
    !s.is_empty() && s.chars().all(f)
}

/// `s.isspace()`.
pub fn isspace(s: &str) -> bool {
    all_nonempty(s, is_space_char)
}

/// `s.isdigit()`.
pub fn isdigit(s: &str) -> bool {
    all_nonempty(s, is_digit_char)
}

/// `s.isalpha()`.
pub fn isalpha(s: &str) -> bool {
    all_nonempty(s, is_alpha_char)
}

/// `s.isalnum()`.
pub fn isalnum(s: &str) -> bool {
    all_nonempty(s, |c| {
        is_alpha_char(c) || is_digit_char(c) || is_numeric_char(c)
    })
}

// ---------------------------------------------------------------- case

/// `s.upper()` (full Unicode mapping: `ß` becomes `SS`).
pub fn upper(s: &str) -> String {
    s.to_uppercase()
}

/// `s.lower()`.
pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// `s.capitalize()` (first char title-cased, the rest lowered).
pub fn capitalize(s: &str) -> String {
    let mut it = s.chars();
    let Some(first) = it.next() else {
        return String::new();
    };
    let mut out = if first == '\u{df}' {
        "Ss".to_owned()
    } else {
        first.to_uppercase().collect()
    };
    out.push_str(&it.as_str().to_lowercase());
    out
}

// ---------------------------------------------------------------- strip / split

/// `s.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space_char)
}

/// `s.lstrip()`.
pub fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_space_char)
}

/// `s.rstrip()`.
pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space_char)
}

/// `s.strip(chars)`.
pub fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}

/// `s.lstrip(chars)`.
pub fn lstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_start_matches(|c| chars.contains(c))
}

/// `s.rstrip(chars)`.
pub fn rstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_end_matches(|c| chars.contains(c))
}

/// `s.split(sep, maxsplit)` with a non-empty `sep` (an empty one is Python's `ValueError`: panics).
pub fn split<'a>(s: &'a str, sep: &str, maxsplit: Option<usize>) -> Vec<&'a str> {
    assert!(!sep.is_empty(), "empty separator");
    match maxsplit {
        Some(n) => s.splitn(n.saturating_add(1), sep).collect(),
        None => s.split(sep).collect(),
    }
}

/// `s.rsplit(sep, maxsplit)` with a non-empty `sep`.
pub fn rsplit<'a>(s: &'a str, sep: &str, maxsplit: Option<usize>) -> Vec<&'a str> {
    assert!(!sep.is_empty(), "empty separator");
    let Some(n) = maxsplit else {
        return split(s, sep, None);
    };
    let mut v: Vec<&str> = s.rsplitn(n.saturating_add(1), sep).collect();
    v.reverse();
    v
}

/// `s.split(None, maxsplit)`: split on whitespace runs; leading whitespace is dropped and the
/// remainder after `maxsplit` splits keeps its trailing whitespace.
pub fn split_ws(s: &str, maxsplit: Option<usize>) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = lstrip(s);
    while !rest.is_empty() {
        if maxsplit.is_some_and(|m| out.len() >= m) {
            out.push(rest);
            break;
        }
        if let Some(i) = rest.find(is_space_char) {
            out.push(&rest[..i]);
            rest = lstrip(&rest[i..]);
        } else {
            out.push(rest);
            break;
        }
    }
    out
}

/// `s.partition(sep)`.
pub fn partition<'a>(s: &'a str, sep: &str) -> (&'a str, &'a str, &'a str) {
    assert!(!sep.is_empty(), "empty separator");
    match s.find(sep) {
        Some(i) => (&s[..i], &s[i..i + sep.len()], &s[i + sep.len()..]),
        None => (s, "", ""),
    }
}

/// `s.rpartition(sep)`.
pub fn rpartition<'a>(s: &'a str, sep: &str) -> (&'a str, &'a str, &'a str) {
    assert!(!sep.is_empty(), "empty separator");
    match s.rfind(sep) {
        Some(i) => (&s[..i], &s[i..i + sep.len()], &s[i + sep.len()..]),
        None => ("", "", s),
    }
}

/// `s.splitlines()` (without line ends).
pub fn splitlines(s: &str) -> Vec<&str> {
    let is_break = |c: char| {
        matches!(
            c,
            '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}'
                ..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
        )
    };
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if is_break(c) {
            out.push(&s[start..i]);
            start = i + c.len_utf8();
            if c == '\r' && it.next_if(|&(_, d)| d == '\n').is_some() {
                start += 1;
            }
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// `s.startswith((p1, p2, ...))`.
pub fn startswith_any(s: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| s.starts_with(p))
}

/// `s.endswith((p1, p2, ...))`.
pub fn endswith_any(s: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|p| s.ends_with(p))
}

// ---------------------------------------------------------------- search

/// Char range `(a, b)` that `find`/`rfind` search, or `None` where Python returns -1 outright.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "bounded by n"
)]
fn search_range(s: &str, start: Option<i64>, end: Option<i64>) -> Option<(usize, usize)> {
    let n = len(s);
    let a = match start {
        None => 0,
        Some(i) if i < 0 => (i + n as i64).max(0) as usize,
        Some(i) => i as usize,
    };
    let b = end.map_or(n, |i| clamp_bound(n, i));
    (a <= n && a <= b).then_some((a, b))
}

fn char_index(s: &str, byte: usize) -> usize {
    if s.is_ascii() {
        byte
    } else {
        s[..byte].chars().count()
    }
}

/// `s.find(sub, start, end)` as `Some(char index)`, `None` for -1.
pub fn find(s: &str, sub: &str, start: Option<i64>, end: Option<i64>) -> Option<usize> {
    let (a, b) = search_range(s, start, end)?;
    let (ba, bb) = (byte_offset(s, a), byte_offset(s, b));
    s[ba..bb].find(sub).map(|i| a + char_index(&s[ba..], i))
}

/// `s.rfind(sub, start, end)`.
pub fn rfind(s: &str, sub: &str, start: Option<i64>, end: Option<i64>) -> Option<usize> {
    let (a, b) = search_range(s, start, end)?;
    let (ba, bb) = (byte_offset(s, a), byte_offset(s, b));
    s[ba..bb].rfind(sub).map(|i| a + char_index(&s[ba..], i))
}

/// `s.index(sub, start, end)`: like [`find`]; `None` is Python's `ValueError`.
pub fn index(s: &str, sub: &str, start: Option<i64>, end: Option<i64>) -> Option<usize> {
    find(s, sub, start, end)
}

/// `s.count(sub)` (non-overlapping; an empty `sub` counts `len(s) + 1`).
pub fn count(s: &str, sub: &str) -> usize {
    s.matches(sub).count()
}

/// `s.replace(old, new)` (all occurrences; identical to the std method).
pub fn replace(s: &str, old: &str, new: &str) -> String {
    s.replace(old, new)
}

// ---------------------------------------------------------------- repr, encodings, format

/// `repr(s)` for a str: quote choice and escapes as Python.
pub fn repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            ' '..='~' => out.push(c),
            c if is_printable(c) => out.push(c),
            c => {
                let cp = u32::from(c);
                let _ = if cp <= 0xff {
                    write!(out, "\\x{cp:02x}")
                } else if cp <= 0xffff {
                    write!(out, "\\u{cp:04x}")
                } else {
                    write!(out, "\\U{cp:08x}")
                };
            }
        }
    }
    out.push(quote);
    out
}

/// `str.isprintable` for one non-ASCII-printable char (exact up to U+00FF).
fn is_printable(c: char) -> bool {
    let cp = u32::from(c);
    if cp < 0x20 || (0x7f..=0xa0).contains(&cp) || cp == 0xad {
        return false;
    }
    !(c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(cp, 0x200b..=0x200f | 0x2028..=0x202e | 0x2060..=0x2064 | 0xfeff))
}

/// `s.encode("latin-1", "replace")`: chars above U+00FF become `?`.
#[allow(clippy::cast_possible_truncation, reason = "guarded to <= 0xFF")]
pub fn encode_latin1_replace(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            if u32::from(c) <= 0xff {
                u32::from(c) as u8
            } else {
                b'?'
            }
        })
        .collect()
}

/// `s.encode("latin-1")`; `None` is Python's `UnicodeEncodeError`.
#[allow(clippy::cast_possible_truncation, reason = "guarded to <= 0xFF")]
pub fn encode_latin1(s: &str) -> Option<Vec<u8>> {
    s.chars()
        .map(|c| (u32::from(c) <= 0xff).then_some(u32::from(c) as u8))
        .collect()
}

/// `b.decode("latin-1")`: one byte, one char.
pub fn decode_latin1(b: &[u8]) -> String {
    b.iter().map(|&x| char::from(x)).collect()
}

/// `template.format(...)` for the `{name}`, `{}` / `{0}` (give those keys `""`/`"0"`, `"1"` ...
/// by position) and `{{` `}}` forms. No conversions or format specs; a missing key panics
/// (Python's `KeyError`).
pub fn format_named(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::new();
    let mut auto = 0usize;
    let mut it = template.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut key = String::new();
                for d in it.by_ref() {
                    if d == '}' {
                        break;
                    }
                    key.push(d);
                }
                if key.is_empty() {
                    key = auto.to_string();
                    auto += 1;
                }
                let v = args
                    .iter()
                    .find(|(k, _)| *k == key)
                    .unwrap_or_else(|| panic!("format key {key:?} missing"));
                out.push_str(v.1);
            }
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------- integer and float arithmetic

/// `int(x)` for a float: truncation toward zero. `None` for nan/inf (Python raises); values
/// beyond i64 saturate (Python would give a big int).
#[allow(
    clippy::cast_possible_truncation,
    reason = "saturating float-to-int is the documented behaviour"
)]
pub fn py_int_from_f64(x: f64) -> Option<i64> {
    x.is_finite().then(|| x.trunc() as i64)
}

/// `round(x)`: banker's rounding to an int. `None` for nan/inf.
#[allow(
    clippy::cast_possible_truncation,
    reason = "saturating float-to-int is the documented behaviour"
)]
pub fn py_round(x: f64) -> Option<i64> {
    x.is_finite().then(|| x.round_ties_even() as i64)
}

/// `a // b` for ints (`None`: `ZeroDivisionError`, or i64 overflow).
pub fn floordiv_i64(a: i64, b: i64) -> Option<i64> {
    let q = a.checked_div(b)?;
    Some(if a % b != 0 && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    })
}

/// `a % b` for ints: the result takes the sign of `b` (`None`: `ZeroDivisionError`).
pub fn pymod_i64(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    if b == -1 {
        return Some(0);
    }
    let r = a % b;
    Some(if r != 0 && ((r < 0) != (b < 0)) {
        r + b
    } else {
        r
    })
}

/// `divmod(a, b)` for ints.
pub fn divmod_i64(a: i64, b: i64) -> Option<(i64, i64)> {
    Some((floordiv_i64(a, b)?, pymod_i64(a, b)?))
}

/// `divmod(a, b)` for floats, as `CPython` computes it (`None`: `ZeroDivisionError`).
pub fn divmod_f64(a: f64, b: f64) -> Option<(f64, f64)> {
    if b == 0.0 {
        return None;
    }
    let mut m = a % b;
    let mut div = (a - m) / b;
    if m == 0.0 {
        m = 0.0_f64.copysign(b);
    } else if (b < 0.0) != (m < 0.0) {
        m += b;
        div -= 1.0;
    }
    let fd = if div == 0.0 {
        0.0_f64.copysign(a / b)
    } else {
        let f = div.floor();
        if div - f > 0.5 { f + 1.0 } else { f }
    };
    Some((fd, m))
}

/// `a // b` for floats.
pub fn floordiv_f64(a: f64, b: f64) -> Option<f64> {
    divmod_f64(a, b).map(|r| r.0)
}

/// `a % b` for floats: the result takes the sign of `b`.
pub fn pymod_f64(a: f64, b: f64) -> Option<f64> {
    divmod_f64(a, b).map(|r| r.1)
}

/// `x.is_integer()`.
pub fn is_integer(x: f64) -> bool {
    x.is_finite() && x.fract() == 0.0
}

// ---------------------------------------------------------------- parsing

/// Digits with single underscores between digits (`1_000`); returns them without underscores.
fn digit_run(s: &str, radix: u32) -> Option<String> {
    let mut out = String::new();
    let mut prev_digit = false;
    for c in s.chars() {
        if c == '_' {
            if !prev_digit {
                return None;
            }
            prev_digit = false;
        } else if c.is_digit(radix) {
            out.push(c);
            prev_digit = true;
        } else {
            return None;
        }
    }
    prev_digit.then_some(out)
}

/// Whitespace `int()`/`float()` strip: `isspace` except the ASCII separators U+001C..U+001F.
fn strip_num_space(s: &str) -> &str {
    s.trim_matches(|c: char| is_space_char(c) && !('\u{1c}'..='\u{1f}').contains(&c))
}

/// `int(s)`: surrounding whitespace, a sign, digits with single underscores. `None` where Python
/// raises `ValueError` (or the value does not fit an i64).
pub fn parse_int(s: &str) -> Option<i64> {
    parse_int_radix(s, 10)
}

/// `int(s, base)` for base 2, 8, 10 or 16 (an optional `0b`/`0o`/`0x` prefix matching the base).
pub fn parse_int_radix(s: &str, base: u32) -> Option<i64> {
    let t = strip_num_space(s);
    let (neg, t) = match t.as_bytes().first()? {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let prefix = match base {
        2 => "0b",
        8 => "0o",
        16 => "0x",
        _ => "",
    };
    let mut body = t;
    if !prefix.is_empty() && t.len() > 2 && t[..2].eq_ignore_ascii_case(prefix) {
        body = &t[2..];
        if let Some(r) = body.strip_prefix('_') {
            body = r;
        }
    }
    let digits = digit_run(body, base)?;
    i64::from_str_radix(&format!("{}{digits}", if neg { "-" } else { "" }), base).ok()
}

/// `float(s)`: whitespace, sign, `inf`/`infinity`/`nan` in any case, decimals with an optional
/// exponent and single underscores between digits. `None` where Python raises `ValueError`.
pub fn parse_float(s: &str) -> Option<f64> {
    let t = strip_num_space(s);
    let (neg, t) = match t.as_bytes().first()? {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let v = if t.eq_ignore_ascii_case("inf") || t.eq_ignore_ascii_case("infinity") {
        f64::INFINITY
    } else if t.eq_ignore_ascii_case("nan") {
        f64::NAN
    } else {
        let (mant, exp) = match t.find(['e', 'E']) {
            Some(i) => (&t[..i], Some(&t[i + 1..])),
            None => (t, None),
        };
        let (int, frac) = mant
            .split_once('.')
            .map_or((mant, None), |(a, b)| (a, Some(b)));
        let int_d = if int.is_empty() {
            String::new()
        } else {
            digit_run(int, 10)?
        };
        let frac_d = match frac {
            Some(f) if !f.is_empty() => digit_run(f, 10)?,
            _ => String::new(),
        };
        if int_d.is_empty() && frac_d.is_empty() {
            return None;
        }
        let mut clean = format!("{int_d}.{frac_d}");
        if let Some(e) = exp {
            let (esign, edigits) = match e.as_bytes().first() {
                Some(b'-') => ("-", &e[1..]),
                Some(b'+') => ("", &e[1..]),
                _ => ("", e),
            };
            write!(clean, "e{esign}{}", digit_run(edigits, 10)?).ok()?;
        }
        if clean.starts_with('.') {
            clean.insert(0, '0');
        }
        clean
            .replace(".e", ".0e")
            .trim_end_matches('.')
            .parse::<f64>()
            .ok()?
    };
    Some(if neg { -v } else { v })
}

// ---------------------------------------------------------------- float formatting

/// Shortest round-trip (or `prec`-digit) decimal digits and decimal exponent of `|x|`
/// (`d.ddd` x 10^exp), from Rust's exact `{:e}` formatting.
fn sci_parts(x: f64, prec: Option<usize>) -> (String, i32) {
    let t = match prec {
        Some(p) => format!("{:.*e}", p, x.abs()),
        None => format!("{:e}", x.abs()),
    };
    let (m, e) = t.split_once('e').unwrap_or((&t, "0"));
    (m.replace('.', ""), e.parse().unwrap_or(0))
}

fn nonfinite(x: f64) -> Option<String> {
    if x.is_nan() {
        Some("nan".to_owned())
    } else if x.is_infinite() {
        Some(if x < 0.0 { "-inf" } else { "inf" }.to_owned())
    } else {
        None
    }
}

fn sign(x: f64) -> &'static str {
    if x.is_sign_negative() { "-" } else { "" }
}

fn sci_text(digits: &str, exp: i32) -> String {
    let (d0, rest) = digits.split_at(1);
    let dot = if rest.is_empty() { "" } else { "." };
    format!(
        "{d0}{dot}{rest}e{}{:02}",
        if exp < 0 { '-' } else { '+' },
        exp.abs()
    )
}

/// `repr(x)` / `str(x)` for a float.
pub fn repr_f64(x: f64) -> String {
    if let Some(s) = nonfinite(x) {
        return s;
    }
    if x == 0.0 {
        return format!("{}0.0", sign(x));
    }
    let (digits, exp) = sci_parts(x, None);
    let decpt = exp + 1;
    let body = if (-3..=16).contains(&decpt) {
        let n = digits.len();
        #[allow(clippy::cast_sign_loss, reason = "branches check the sign")]
        if decpt <= 0 {
            format!("0.{}{digits}", "0".repeat((-decpt) as usize))
        } else if n <= decpt as usize {
            format!("{digits}{}.0", "0".repeat(decpt as usize - n))
        } else {
            format!(
                "{}.{}",
                &digits[..decpt as usize],
                &digits[decpt as usize..]
            )
        }
    } else {
        sci_text(&digits, exp)
    };
    format!("{}{body}", sign(x))
}

/// `format(x, ".{prec}e")` / `"%.{prec}e" % x`.
pub fn format_e(x: f64, prec: usize) -> String {
    if let Some(s) = nonfinite(x) {
        return s;
    }
    let (digits, exp) = sci_parts(x, Some(prec));
    format!("{}{}", sign(x), sci_text(&digits, exp))
}

/// `format(x, ".{prec}f")` / `"%.{prec}f" % x`.
pub fn format_f(x: f64, prec: usize) -> String {
    nonfinite(x).unwrap_or_else(|| format!("{x:.prec$}"))
}

/// `format(x, ".{prec}g")` / `"%.{prec}g" % x`; `prec` 6 is plain `:g` / `%g`.
pub fn format_g(x: f64, prec: usize) -> String {
    if let Some(s) = nonfinite(x) {
        return s;
    }
    let p = prec.max(1);
    if x == 0.0 {
        return format!("{}0", sign(x));
    }
    let (digits, exp) = sci_parts(x, Some(p - 1));
    let trimmed = digits.trim_end_matches('0');
    let digits = if trimmed.is_empty() { "0" } else { trimmed };
    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        reason = "p is a small precision; branches check the sign"
    )]
    let body = if exp < -4 || exp >= p as i32 {
        sci_text(digits, exp)
    } else if exp < 0 {
        format!("0.{}{digits}", "0".repeat((-exp - 1) as usize))
    } else if digits.len() <= exp as usize + 1 {
        format!("{digits}{}", "0".repeat(exp as usize + 1 - digits.len()))
    } else {
        format!(
            "{}.{}",
            &digits[..=exp as usize],
            &digits[exp as usize + 1..]
        )
    };
    format!("{}{body}", sign(x))
}

#[cfg(test)]
#[allow(
    clippy::unreadable_literal,
    reason = "expected values are CPython output, pasted verbatim"
)]
mod tests {
    use super::*;

    fn feq(a: Option<f64>, b: Option<f64>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()),
            _ => false,
        }
    }

    /// Expected values in this module were produced by running Python on the same inputs.
    #[test]
    fn python_str_cases() {
        assert_eq!(len(""), 0);
        assert_eq!(len("abc"), 3);
        assert_eq!(len("\u{e9}\u{b0}x"), 3);
        assert_eq!(len("\u{394}\u{3a9}"), 2);
        assert_eq!(char_at("", 0), None);
        assert_eq!(char_at("", 2), None);
        assert_eq!(char_at("", 3), None);
        assert_eq!(char_at("", -1), None);
        assert_eq!(char_at("", -3), None);
        assert_eq!(char_at("", -4), None);
        assert_eq!(char_at("", 10), None);
        assert_eq!(char_at("abc", 0), Some('a'));
        assert_eq!(char_at("abc", 2), Some('c'));
        assert_eq!(char_at("abc", 3), None);
        assert_eq!(char_at("abc", -1), Some('c'));
        assert_eq!(char_at("abc", -3), Some('a'));
        assert_eq!(char_at("abc", -4), None);
        assert_eq!(char_at("abc", 10), None);
        assert_eq!(char_at("\u{e9}\u{b0}x", 0), Some('\u{e9}'));
        assert_eq!(char_at("\u{e9}\u{b0}x", 2), Some('x'));
        assert_eq!(char_at("\u{e9}\u{b0}x", 3), None);
        assert_eq!(char_at("\u{e9}\u{b0}x", -1), Some('x'));
        assert_eq!(char_at("\u{e9}\u{b0}x", -3), Some('\u{e9}'));
        assert_eq!(char_at("\u{e9}\u{b0}x", -4), None);
        assert_eq!(char_at("\u{e9}\u{b0}x", 10), None);
        assert_eq!(slice("hello", None, None), "hello");
        assert_eq!(slice("hello", Some(0), Some(2)), "he");
        assert_eq!(slice("hello", Some(1), None), "ello");
        assert_eq!(slice("hello", None, Some(-1)), "hell");
        assert_eq!(slice("hello", Some(-2), None), "lo");
        assert_eq!(slice("hello", Some(-3), Some(-1)), "ll");
        assert_eq!(slice("hello", Some(2), Some(1)), "");
        assert_eq!(slice("hello", Some(10), Some(20)), "");
        assert_eq!(slice("hello", Some(-10), Some(2)), "he");
        assert_eq!(slice("hello", Some(1), Some(-10)), "");
        assert_eq!(slice("hello", Some(3), Some(3)), "");
        assert_eq!(slice("hello", Some(0), Some(99)), "hello");
        assert_eq!(slice("hello", Some(-1), Some(-99)), "");
        assert_eq!(slice("h\u{e9}llo\u{b0}", None, None), "h\u{e9}llo\u{b0}");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(0), Some(2)), "h\u{e9}");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(1), None), "\u{e9}llo\u{b0}");
        assert_eq!(slice("h\u{e9}llo\u{b0}", None, Some(-1)), "h\u{e9}llo");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(-2), None), "o\u{b0}");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(-3), Some(-1)), "lo");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(2), Some(1)), "");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(10), Some(20)), "");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(-10), Some(2)), "h\u{e9}");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(1), Some(-10)), "");
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(3), Some(3)), "");
        assert_eq!(
            slice("h\u{e9}llo\u{b0}", Some(0), Some(99)),
            "h\u{e9}llo\u{b0}"
        );
        assert_eq!(slice("h\u{e9}llo\u{b0}", Some(-1), Some(-99)), "");
        assert_eq!(slice("", None, None), "");
        assert_eq!(slice("", Some(0), Some(2)), "");
        assert_eq!(slice("", Some(1), None), "");
        assert_eq!(slice("", None, Some(-1)), "");
        assert_eq!(slice("", Some(-2), None), "");
        assert_eq!(slice("", Some(-3), Some(-1)), "");
        assert_eq!(slice("", Some(2), Some(1)), "");
        assert_eq!(slice("", Some(10), Some(20)), "");
        assert_eq!(slice("", Some(-10), Some(2)), "");
        assert_eq!(slice("", Some(1), Some(-10)), "");
        assert_eq!(slice("", Some(3), Some(3)), "");
        assert_eq!(slice("", Some(0), Some(99)), "");
        assert_eq!(slice("", Some(-1), Some(-99)), "");
        assert_eq!(upper("abc"), "ABC");
        assert_eq!(lower("abc"), "abc");
        assert_eq!(upper("Stra\u{df}e"), "STRASSE");
        assert_eq!(lower("Stra\u{df}e"), "stra\u{df}e");
        assert_eq!(
            upper("\u{e9}\u{ff}\u{b5}\u{aa}"),
            "\u{c9}\u{178}\u{39c}\u{aa}"
        );
        assert_eq!(
            lower("\u{e9}\u{ff}\u{b5}\u{aa}"),
            "\u{e9}\u{ff}\u{b5}\u{aa}"
        );
        assert_eq!(upper("\u{130}"), "\u{130}");
        assert_eq!(lower("\u{130}"), "i\u{307}");
        assert_eq!(upper("\u{3a3}"), "\u{3a3}");
        assert_eq!(lower("\u{3a3}"), "\u{3c3}");
        assert_eq!(upper("x\u{b0}y"), "X\u{b0}Y");
        assert_eq!(lower("x\u{b0}y"), "x\u{b0}y");
        assert_eq!(upper(""), "");
        assert_eq!(lower(""), "");
        assert_eq!(upper("\u{df}"), "SS");
        assert_eq!(lower("\u{df}"), "\u{df}");
        assert_eq!(
            upper("\u{391}\u{3a3} \u{391}\u{3a3}"),
            "\u{391}\u{3a3} \u{391}\u{3a3}"
        );
        assert_eq!(
            lower("\u{391}\u{3a3} \u{391}\u{3a3}"),
            "\u{3b1}\u{3c2} \u{3b1}\u{3c2}"
        );
        assert_eq!(upper("\u{391}\u{3a3}\u{b0}"), "\u{391}\u{3a3}\u{b0}");
        assert_eq!(lower("\u{391}\u{3a3}\u{b0}"), "\u{3b1}\u{3c2}\u{b0}");
        assert_eq!(upper("A\u{3a3}"), "A\u{3a3}");
        assert_eq!(lower("A\u{3a3}"), "a\u{3c2}");
        assert_eq!(capitalize("hello WORLD"), "Hello world");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("\u{df}test"), "Sstest");
        assert_eq!(capitalize("\u{e9}COLE"), "\u{c9}cole");
        assert_eq!(capitalize("1ABC"), "1abc");
        assert_eq!(capitalize("\u{ff}\u{e9}"), "\u{178}\u{e9}");
        assert_eq!(capitalize("a"), "A");
        assert_eq!(strip(" \t x \n"), "x");
        assert_eq!(lstrip(" \t x \n"), "x \n");
        assert_eq!(rstrip(" \t x \n"), " \t x");
        assert_eq!(strip("\u{1c}\u{1f}x\u{85}\u{a0}"), "x");
        assert_eq!(lstrip("\u{1c}\u{1f}x\u{85}\u{a0}"), "x\u{85}\u{a0}");
        assert_eq!(rstrip("\u{1c}\u{1f}x\u{85}\u{a0}"), "\u{1c}\u{1f}x");
        assert_eq!(strip(""), "");
        assert_eq!(lstrip(""), "");
        assert_eq!(rstrip(""), "");
        assert_eq!(strip("   "), "");
        assert_eq!(lstrip("   "), "");
        assert_eq!(rstrip("   "), "");
        assert_eq!(strip("x"), "x");
        assert_eq!(lstrip("x"), "x");
        assert_eq!(rstrip("x"), "x");
        assert_eq!(strip("\u{a0}\u{e9}\u{a0}"), "\u{e9}");
        assert_eq!(lstrip("\u{a0}\u{e9}\u{a0}"), "\u{e9}\u{a0}");
        assert_eq!(rstrip("\u{a0}\u{e9}\u{a0}"), "\u{a0}\u{e9}");
        assert_eq!(strip("\u{b}\u{c}x\r"), "x");
        assert_eq!(lstrip("\u{b}\u{c}x\r"), "x\r");
        assert_eq!(rstrip("\u{b}\u{c}x\r"), "\u{b}\u{c}x");
        assert_eq!(strip("\u{2003}x\u{3000}"), "x");
        assert_eq!(lstrip("\u{2003}x\u{3000}"), "x\u{3000}");
        assert_eq!(rstrip("\u{2003}x\u{3000}"), "\u{2003}x");
        assert_eq!(strip("\u{0}x\u{0}"), "\u{0}x\u{0}");
        assert_eq!(lstrip("\u{0}x\u{0}"), "\u{0}x\u{0}");
        assert_eq!(rstrip("\u{0}x\u{0}"), "\u{0}x\u{0}");
        assert_eq!(strip_chars("0012300", "0"), "123");
        assert_eq!(lstrip_chars("0012300", "0"), "12300");
        assert_eq!(rstrip_chars("0012300", "0"), "00123");
        assert_eq!(strip_chars(" \tab \t", " \t"), "ab");
        assert_eq!(lstrip_chars(" \tab \t", " \t"), "ab \t");
        assert_eq!(rstrip_chars(" \tab \t", " \t"), " \tab");
        assert_eq!(strip_chars("&H1", "&"), "H1");
        assert_eq!(lstrip_chars("&H1", "&"), "H1");
        assert_eq!(rstrip_chars("&H1", "&"), "&H1");
        assert_eq!(strip_chars(",a,,", ","), "a");
        assert_eq!(lstrip_chars(",a,,", ","), "a,,");
        assert_eq!(rstrip_chars(",a,,", ","), ",a");
        assert_eq!(strip_chars("abc", ""), "abc");
        assert_eq!(lstrip_chars("abc", ""), "abc");
        assert_eq!(rstrip_chars("abc", ""), "abc");
        assert_eq!(strip_chars("aXb", "ab"), "X");
        assert_eq!(lstrip_chars("aXb", "ab"), "Xb");
        assert_eq!(rstrip_chars("aXb", "ab"), "aX");
        assert_eq!(strip_chars("0123abc", "0123456789"), "abc");
        assert_eq!(lstrip_chars("0123abc", "0123456789"), "abc");
        assert_eq!(rstrip_chars("0123abc", "0123456789"), "0123abc");
        assert_eq!(strip_chars("\u{e9}x\u{e9}", "\u{e9}"), "x");
        assert_eq!(lstrip_chars("\u{e9}x\u{e9}", "\u{e9}"), "x\u{e9}");
        assert_eq!(rstrip_chars("\u{e9}x\u{e9}", "\u{e9}"), "\u{e9}x");
        assert_eq!(strip_chars("", "a"), "");
        assert_eq!(lstrip_chars("", "a"), "");
        assert_eq!(rstrip_chars("", "a"), "");
        assert_eq!(strip_chars("aaa", "a"), "");
        assert_eq!(lstrip_chars("aaa", "a"), "");
        assert_eq!(rstrip_chars("aaa", "a"), "");
        assert_eq!(split("a,b,,c", ",", None), vec!["a", "b", "", "c"]);
        assert_eq!(split("", ",", None), vec![""]);
        assert_eq!(split("a=b=c", "=", Some(1)), vec!["a", "b=c"]);
        assert_eq!(split("abc", "x", None), vec!["abc"]);
        assert_eq!(split("a,b,", ",", None), vec!["a", "b", ""]);
        assert_eq!(split("a,,b", ",", Some(1)), vec!["a", ",b"]);
        assert_eq!(split("\u{e9},\u{b0}", ",", None), vec!["\u{e9}", "\u{b0}"]);
        assert_eq!(split("a::b::c", "::", Some(1)), vec!["a", "b::c"]);
        assert_eq!(split("a\nb\n", "\n", None), vec!["a", "b", ""]);
        assert_eq!(split("a=b", "=", Some(0)), vec!["a=b"]);
        assert_eq!(split("a,b,c", ",", Some(9)), vec!["a", "b", "c"]);
        assert_eq!(rsplit("a/b/c", "/", Some(1)), vec!["a/b", "c"]);
        assert_eq!(rsplit("a/b/c", "/", None), vec!["a", "b", "c"]);
        assert_eq!(rsplit("abc", "/", Some(1)), vec!["abc"]);
        assert_eq!(rsplit("a/b/c", "/", Some(5)), vec!["a", "b", "c"]);
        assert_eq!(rsplit("/a", "/", Some(1)), vec!["", "a"]);
        assert_eq!(rsplit("a//b", "//", None), vec!["a", "b"]);
        assert_eq!(rsplit("a/b/c", "/", Some(0)), vec!["a/b/c"]);
        assert_eq!(
            rsplit("\u{e9}/\u{b0}/x", "/", Some(1)),
            vec!["\u{e9}/\u{b0}", "x"]
        );
        assert_eq!(split_ws("  a  b  c  ", None), vec!["a", "b", "c"]);
        assert_eq!(split_ws("a b c", Some(1)), vec!["a", "b c"]);
        assert_eq!(split_ws(" a b ", Some(1)), vec!["a", "b "]);
        assert_eq!(split_ws("a", Some(1)), vec!["a"]);
        assert_eq!(split_ws("", None), Vec::<&str>::new());
        assert_eq!(split_ws(" ", Some(1)), Vec::<&str>::new());
        assert_eq!(split_ws("a\u{a0}b\u{1c}c", None), vec!["a", "b", "c"]);
        assert_eq!(split_ws("a  b  ", Some(0)), vec!["a  b  "]);
        assert_eq!(split_ws("  a b", Some(0)), vec!["a b"]);
        assert_eq!(split_ws("a b c", Some(2)), vec!["a", "b", "c"]);
        assert_eq!(split_ws("a b c", Some(5)), vec!["a", "b", "c"]);
        assert_eq!(split_ws("x\ty  z", Some(1)), vec!["x", "y  z"]);
        assert_eq!(split_ws("a b  ", Some(1)), vec!["a", "b  "]);
        assert_eq!(split_ws("a  ", Some(1)), vec!["a"]);
        assert_eq!(partition("a:b:c", ":"), ("a", ":", "b:c"));
        assert_eq!(rpartition("a:b:c", ":"), ("a:b", ":", "c"));
        assert_eq!(partition("abc", ":"), ("abc", "", ""));
        assert_eq!(rpartition("abc", ":"), ("", "", "abc"));
        assert_eq!(partition("", ":"), ("", "", ""));
        assert_eq!(rpartition("", ":"), ("", "", ""));
        assert_eq!(partition(":", ":"), ("", ":", ""));
        assert_eq!(rpartition(":", ":"), ("", ":", ""));
        assert_eq!(partition("a::b", "::"), ("a", "::", "b"));
        assert_eq!(rpartition("a::b", "::"), ("a", "::", "b"));
        assert_eq!(partition("\u{e9}:\u{b0}", ":"), ("\u{e9}", ":", "\u{b0}"));
        assert_eq!(rpartition("\u{e9}:\u{b0}", ":"), ("\u{e9}", ":", "\u{b0}"));
        assert_eq!(partition("a.b.c", "."), ("a", ".", "b.c"));
        assert_eq!(rpartition("a.b.c", "."), ("a.b", ".", "c"));
        assert_eq!(splitlines(""), Vec::<&str>::new());
        assert_eq!(splitlines("a"), vec!["a"]);
        assert_eq!(splitlines("a\n"), vec!["a"]);
        assert_eq!(splitlines("a\nb"), vec!["a", "b"]);
        assert_eq!(
            splitlines("a\r\nb\rc\u{b}d\u{c}e\u{1c}f\u{1d}g\u{1e}h\u{85}i\u{2028}j\u{2029}k"),
            vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"]
        );
        assert_eq!(splitlines("\n\n"), vec!["", ""]);
        assert_eq!(splitlines("a\r\n"), vec!["a"]);
        assert_eq!(splitlines("\r\n\r\n"), vec!["", ""]);
        assert_eq!(splitlines("\u{e9}\n\u{b0}"), vec!["\u{e9}", "\u{b0}"]);
        assert_eq!(splitlines("a\u{1f}b"), vec!["a\u{1f}b"]);
        assert!(startswith_any("tier_b1", &["tier_a", "tier_b"]));
        assert!(!endswith_any("tier_b1", &["tier_a", "tier_b"]));
        assert!(!startswith_any("x", &["a", "b"]));
        assert!(!endswith_any("x", &["a", "b"]));
        assert!(startswith_any("", &[""]));
        assert!(endswith_any("", &[""]));
        assert!(!startswith_any("abc", &[]));
        assert!(!endswith_any("abc", &[]));
        assert!(startswith_any("\u{e9}a", &["\u{e9}", "z"]));
        assert!(!endswith_any("\u{e9}a", &["\u{e9}", "z"]));
        assert_eq!(find("a=b=c", "=", None, None), Some(1));
        assert_eq!(rfind("a=b=c", "=", None, None), Some(3));
        assert_eq!(find("a=b=c", "=", Some(2), None), Some(3));
        assert_eq!(rfind("a=b=c", "=", Some(2), None), Some(3));
        assert_eq!(find("a=b=c", "=", Some(-1), None), None);
        assert_eq!(rfind("a=b=c", "=", Some(-1), None), None);
        assert_eq!(find("abc", "", None, None), Some(0));
        assert_eq!(rfind("abc", "", None, None), Some(3));
        assert_eq!(find("abc", "", Some(3), None), Some(3));
        assert_eq!(rfind("abc", "", Some(3), None), Some(3));
        assert_eq!(find("abc", "", Some(4), None), None);
        assert_eq!(rfind("abc", "", Some(4), None), None);
        assert_eq!(find("abc", "", Some(2), Some(1)), None);
        assert_eq!(rfind("abc", "", Some(2), Some(1)), None);
        assert_eq!(find("abc", "c", None, Some(2)), None);
        assert_eq!(rfind("abc", "c", None, Some(2)), None);
        assert_eq!(find("abc", "bc", Some(1), Some(3)), Some(1));
        assert_eq!(rfind("abc", "bc", Some(1), Some(3)), Some(1));
        assert_eq!(find("abc", "bc", Some(1), Some(2)), None);
        assert_eq!(rfind("abc", "bc", Some(1), Some(2)), None);
        assert_eq!(find("\u{e9}\u{e9}x", "x", None, None), Some(2));
        assert_eq!(rfind("\u{e9}\u{e9}x", "x", None, None), Some(2));
        assert_eq!(
            find("\u{e9}\u{e9}x\u{e9}", "\u{e9}", Some(1), None),
            Some(1)
        );
        assert_eq!(
            rfind("\u{e9}\u{e9}x\u{e9}", "\u{e9}", Some(1), None),
            Some(3)
        );
        assert_eq!(find("", "", None, None), Some(0));
        assert_eq!(rfind("", "", None, None), Some(0));
        assert_eq!(find("", "a", None, None), None);
        assert_eq!(rfind("", "a", None, None), None);
        assert_eq!(find("abc", "a", Some(-10), None), Some(0));
        assert_eq!(rfind("abc", "a", Some(-10), None), Some(0));
        assert_eq!(find("abc", "c", Some(-1), None), Some(2));
        assert_eq!(rfind("abc", "c", Some(-1), None), Some(2));
        assert_eq!(find("abc", "a", Some(1), Some(-1)), None);
        assert_eq!(rfind("abc", "a", Some(1), Some(-1)), None);
        assert_eq!(find("abc", "b", Some(0), Some(-1)), Some(1));
        assert_eq!(rfind("abc", "b", Some(0), Some(-1)), Some(1));
        assert_eq!(find("abcabc", "bc", Some(2), None), Some(4));
        assert_eq!(rfind("abcabc", "bc", Some(2), None), Some(4));
        assert_eq!(find("say \"hi\" \"x\"", "\"", Some(5), None), Some(7));
        assert_eq!(rfind("say \"hi\" \"x\"", "\"", Some(5), None), Some(11));
        assert_eq!(find("\u{e9}a\u{e9}a", "a", Some(2), Some(4)), Some(3));
        assert_eq!(rfind("\u{e9}a\u{e9}a", "a", Some(2), Some(4)), Some(3));
        assert_eq!(find("abc", "", None, Some(1)), Some(0));
        assert_eq!(rfind("abc", "", None, Some(1)), Some(1));
        assert_eq!(find("abc", "", Some(-1), None), Some(2));
        assert_eq!(rfind("abc", "", Some(-1), None), Some(3));
        assert_eq!(index("abc", "c", None, None), Some(2));
        assert_eq!(index("abc", "z", None, None), None);
        assert_eq!(index("\u{e9}\u{b0}x", "x", None, None), Some(2));
        assert_eq!(count("a,b,c", ","), 2);
        assert_eq!(count("aaa", "aa"), 1);
        assert_eq!(count("abc", ""), 4);
        assert_eq!(count("", ""), 1);
        assert_eq!(count("\u{e9}\u{e9}\u{e9}", "\u{e9}"), 3);
        assert_eq!(count("a{b}{", "{"), 2);
        assert_eq!(count("\"a\"b\"", "\""), 3);
        assert_eq!(count("abab", "ab"), 2);
        assert_eq!(replace("a\r\nb\r\n", "\r\n", "\n"), "a\nb\n");
        assert_eq!(replace("a\\b", "\\", "/"), "a/b");
        assert_eq!(replace("abc", "", "-"), "-a-b-c-");
        assert_eq!(replace("aaa", "a", "bb"), "bbbbbb");
        assert_eq!(replace("", "a", "b"), "");
        assert_eq!(replace("\u{e9}x\u{e9}", "\u{e9}", "e"), "exe");
        assert_eq!(replace("", "", "x"), "x");
        assert_eq!(repr_str("abc"), "'abc'");
        assert_eq!(repr_str("it's"), "\"it's\"");
        assert_eq!(repr_str("say \"hi\""), "'say \"hi\"'");
        assert_eq!(repr_str("it's \"x\""), "'it\\'s \"x\"'");
        assert_eq!(repr_str("a\\b"), "'a\\\\b'");
        assert_eq!(repr_str("a\nb\tc\r"), "'a\\nb\\tc\\r'");
        assert_eq!(repr_str("\u{0}\u{1f}\u{7f}"), "'\\x00\\x1f\\x7f'");
        assert_eq!(
            repr_str("\u{80}\u{9f}\u{a0}\u{ad}\u{e9}\u{b0}"),
            "'\\x80\\x9f\\xa0\\xad\u{e9}\u{b0}'"
        );
        assert_eq!(repr_str("\u{394}"), "'\u{394}'");
        assert_eq!(repr_str(""), "''");
        assert_eq!(repr_str("'"), "\"'\"");
        assert_eq!(repr_str("\""), "'\"'");
        assert_eq!(repr_str("\u{2028}x"), "'\\u2028x'");
        assert_eq!(repr_str("\u{1f600}"), "'\u{1f600}'");
        assert_eq!(repr_str("\u{200b}"), "'\\u200b'");
        assert_eq!(repr_str("''\""), "'\\'\\'\"'");
        assert_eq!(repr_str("\u{e9}'"), "\"\u{e9}'\"");
        assert_eq!(
            encode_latin1_replace("a\u{e9}\u{394}\u{1f600}\u{ff}"),
            vec![97, 233, 63, 63, 255]
        );
        assert_eq!(encode_latin1("a\u{e9}\u{ff}"), Some(vec![97, 233, 255]));
        assert_eq!(encode_latin1("a\u{394}"), None);
        assert_eq!(
            decode_latin1(&[
                0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255
            ]),
            "\u{0}\u{11}\"3DUfw\u{88}\u{99}\u{aa}\u{bb}\u{cc}\u{dd}\u{ee}\u{ff}"
        );
        assert_eq!(
            format_named("a {x} {{b}} {} {}", &[("x", "X"), ("0", "p"), ("1", "q")]),
            "a X {b} p q"
        );
        assert_eq!(
            format_named("{v}: {name}! {{}}", &[("v", "5"), ("name", "N")]),
            "5: N! {}"
        );
        assert_eq!(
            format_named("Allocated {0}{0}", &[("0", "é")]),
            "Allocated \u{e9}\u{e9}"
        );
    }

    fn per_char(f: impl Fn(&str) -> String) -> String {
        (0u32..256)
            .filter_map(char::from_u32)
            .map(|c| f(&c.to_string()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn bits(f: impl Fn(&str) -> bool) -> String {
        (0u32..256)
            .filter_map(char::from_u32)
            .map(|c| if f(&c.to_string()) { '1' } else { '0' })
            .collect()
    }

    /// Every char U+0000..U+00FF against Python's `str` methods.
    #[test]
    fn latin1_char_tables() {
        let tables: [(&str, &str); 7] = [
            (
                "upper",
                "\u{0}\n\u{1}\n\u{2}\n\u{3}\n\u{4}\n\u{5}\n\u{6}\n\u{7}\n\u{8}\n\t\n\n\n\u{b}\n\u{c}\n\r\n\u{e}\n\u{f}\n\u{10}\n\u{11}\n\u{12}\n\u{13}\n\u{14}\n\u{15}\n\u{16}\n\u{17}\n\u{18}\n\u{19}\n\u{1a}\n\u{1b}\n\u{1c}\n\u{1d}\n\u{1e}\n\u{1f}\n \n!\n\"\n#\n$\n%\n&\n'\n(\n)\n*\n+\n,\n-\n.\n/\n0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n:\n;\n<\n=\n>\n?\n@\nA\nB\nC\nD\nE\nF\nG\nH\nI\nJ\nK\nL\nM\nN\nO\nP\nQ\nR\nS\nT\nU\nV\nW\nX\nY\nZ\n[\n\\\n]\n^\n_\n`\nA\nB\nC\nD\nE\nF\nG\nH\nI\nJ\nK\nL\nM\nN\nO\nP\nQ\nR\nS\nT\nU\nV\nW\nX\nY\nZ\n{\n|\n}\n~\n\u{7f}\n\u{80}\n\u{81}\n\u{82}\n\u{83}\n\u{84}\n\u{85}\n\u{86}\n\u{87}\n\u{88}\n\u{89}\n\u{8a}\n\u{8b}\n\u{8c}\n\u{8d}\n\u{8e}\n\u{8f}\n\u{90}\n\u{91}\n\u{92}\n\u{93}\n\u{94}\n\u{95}\n\u{96}\n\u{97}\n\u{98}\n\u{99}\n\u{9a}\n\u{9b}\n\u{9c}\n\u{9d}\n\u{9e}\n\u{9f}\n\u{a0}\n\u{a1}\n\u{a2}\n\u{a3}\n\u{a4}\n\u{a5}\n\u{a6}\n\u{a7}\n\u{a8}\n\u{a9}\n\u{aa}\n\u{ab}\n\u{ac}\n\u{ad}\n\u{ae}\n\u{af}\n\u{b0}\n\u{b1}\n\u{b2}\n\u{b3}\n\u{b4}\n\u{39c}\n\u{b6}\n\u{b7}\n\u{b8}\n\u{b9}\n\u{ba}\n\u{bb}\n\u{bc}\n\u{bd}\n\u{be}\n\u{bf}\n\u{c0}\n\u{c1}\n\u{c2}\n\u{c3}\n\u{c4}\n\u{c5}\n\u{c6}\n\u{c7}\n\u{c8}\n\u{c9}\n\u{ca}\n\u{cb}\n\u{cc}\n\u{cd}\n\u{ce}\n\u{cf}\n\u{d0}\n\u{d1}\n\u{d2}\n\u{d3}\n\u{d4}\n\u{d5}\n\u{d6}\n\u{d7}\n\u{d8}\n\u{d9}\n\u{da}\n\u{db}\n\u{dc}\n\u{dd}\n\u{de}\nSS\n\u{c0}\n\u{c1}\n\u{c2}\n\u{c3}\n\u{c4}\n\u{c5}\n\u{c6}\n\u{c7}\n\u{c8}\n\u{c9}\n\u{ca}\n\u{cb}\n\u{cc}\n\u{cd}\n\u{ce}\n\u{cf}\n\u{d0}\n\u{d1}\n\u{d2}\n\u{d3}\n\u{d4}\n\u{d5}\n\u{d6}\n\u{f7}\n\u{d8}\n\u{d9}\n\u{da}\n\u{db}\n\u{dc}\n\u{dd}\n\u{de}\n\u{178}",
            ),
            (
                "lower",
                "\u{0}\n\u{1}\n\u{2}\n\u{3}\n\u{4}\n\u{5}\n\u{6}\n\u{7}\n\u{8}\n\t\n\n\n\u{b}\n\u{c}\n\r\n\u{e}\n\u{f}\n\u{10}\n\u{11}\n\u{12}\n\u{13}\n\u{14}\n\u{15}\n\u{16}\n\u{17}\n\u{18}\n\u{19}\n\u{1a}\n\u{1b}\n\u{1c}\n\u{1d}\n\u{1e}\n\u{1f}\n \n!\n\"\n#\n$\n%\n&\n'\n(\n)\n*\n+\n,\n-\n.\n/\n0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n:\n;\n<\n=\n>\n?\n@\na\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nr\ns\nt\nu\nv\nw\nx\ny\nz\n[\n\\\n]\n^\n_\n`\na\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\nq\nr\ns\nt\nu\nv\nw\nx\ny\nz\n{\n|\n}\n~\n\u{7f}\n\u{80}\n\u{81}\n\u{82}\n\u{83}\n\u{84}\n\u{85}\n\u{86}\n\u{87}\n\u{88}\n\u{89}\n\u{8a}\n\u{8b}\n\u{8c}\n\u{8d}\n\u{8e}\n\u{8f}\n\u{90}\n\u{91}\n\u{92}\n\u{93}\n\u{94}\n\u{95}\n\u{96}\n\u{97}\n\u{98}\n\u{99}\n\u{9a}\n\u{9b}\n\u{9c}\n\u{9d}\n\u{9e}\n\u{9f}\n\u{a0}\n\u{a1}\n\u{a2}\n\u{a3}\n\u{a4}\n\u{a5}\n\u{a6}\n\u{a7}\n\u{a8}\n\u{a9}\n\u{aa}\n\u{ab}\n\u{ac}\n\u{ad}\n\u{ae}\n\u{af}\n\u{b0}\n\u{b1}\n\u{b2}\n\u{b3}\n\u{b4}\n\u{b5}\n\u{b6}\n\u{b7}\n\u{b8}\n\u{b9}\n\u{ba}\n\u{bb}\n\u{bc}\n\u{bd}\n\u{be}\n\u{bf}\n\u{e0}\n\u{e1}\n\u{e2}\n\u{e3}\n\u{e4}\n\u{e5}\n\u{e6}\n\u{e7}\n\u{e8}\n\u{e9}\n\u{ea}\n\u{eb}\n\u{ec}\n\u{ed}\n\u{ee}\n\u{ef}\n\u{f0}\n\u{f1}\n\u{f2}\n\u{f3}\n\u{f4}\n\u{f5}\n\u{f6}\n\u{d7}\n\u{f8}\n\u{f9}\n\u{fa}\n\u{fb}\n\u{fc}\n\u{fd}\n\u{fe}\n\u{df}\n\u{e0}\n\u{e1}\n\u{e2}\n\u{e3}\n\u{e4}\n\u{e5}\n\u{e6}\n\u{e7}\n\u{e8}\n\u{e9}\n\u{ea}\n\u{eb}\n\u{ec}\n\u{ed}\n\u{ee}\n\u{ef}\n\u{f0}\n\u{f1}\n\u{f2}\n\u{f3}\n\u{f4}\n\u{f5}\n\u{f6}\n\u{f7}\n\u{f8}\n\u{f9}\n\u{fa}\n\u{fb}\n\u{fc}\n\u{fd}\n\u{fe}\n\u{ff}",
            ),
            (
                "repr",
                "'\\x00'\n'\\x01'\n'\\x02'\n'\\x03'\n'\\x04'\n'\\x05'\n'\\x06'\n'\\x07'\n'\\x08'\n'\\t'\n'\\n'\n'\\x0b'\n'\\x0c'\n'\\r'\n'\\x0e'\n'\\x0f'\n'\\x10'\n'\\x11'\n'\\x12'\n'\\x13'\n'\\x14'\n'\\x15'\n'\\x16'\n'\\x17'\n'\\x18'\n'\\x19'\n'\\x1a'\n'\\x1b'\n'\\x1c'\n'\\x1d'\n'\\x1e'\n'\\x1f'\n' '\n'!'\n'\"'\n'#'\n'$'\n'%'\n'&'\n\"'\"\n'('\n')'\n'*'\n'+'\n','\n'-'\n'.'\n'/'\n'0'\n'1'\n'2'\n'3'\n'4'\n'5'\n'6'\n'7'\n'8'\n'9'\n':'\n';'\n'<'\n'='\n'>'\n'?'\n'@'\n'A'\n'B'\n'C'\n'D'\n'E'\n'F'\n'G'\n'H'\n'I'\n'J'\n'K'\n'L'\n'M'\n'N'\n'O'\n'P'\n'Q'\n'R'\n'S'\n'T'\n'U'\n'V'\n'W'\n'X'\n'Y'\n'Z'\n'['\n'\\\\'\n']'\n'^'\n'_'\n'`'\n'a'\n'b'\n'c'\n'd'\n'e'\n'f'\n'g'\n'h'\n'i'\n'j'\n'k'\n'l'\n'm'\n'n'\n'o'\n'p'\n'q'\n'r'\n's'\n't'\n'u'\n'v'\n'w'\n'x'\n'y'\n'z'\n'{'\n'|'\n'}'\n'~'\n'\\x7f'\n'\\x80'\n'\\x81'\n'\\x82'\n'\\x83'\n'\\x84'\n'\\x85'\n'\\x86'\n'\\x87'\n'\\x88'\n'\\x89'\n'\\x8a'\n'\\x8b'\n'\\x8c'\n'\\x8d'\n'\\x8e'\n'\\x8f'\n'\\x90'\n'\\x91'\n'\\x92'\n'\\x93'\n'\\x94'\n'\\x95'\n'\\x96'\n'\\x97'\n'\\x98'\n'\\x99'\n'\\x9a'\n'\\x9b'\n'\\x9c'\n'\\x9d'\n'\\x9e'\n'\\x9f'\n'\\xa0'\n'\u{a1}'\n'\u{a2}'\n'\u{a3}'\n'\u{a4}'\n'\u{a5}'\n'\u{a6}'\n'\u{a7}'\n'\u{a8}'\n'\u{a9}'\n'\u{aa}'\n'\u{ab}'\n'\u{ac}'\n'\\xad'\n'\u{ae}'\n'\u{af}'\n'\u{b0}'\n'\u{b1}'\n'\u{b2}'\n'\u{b3}'\n'\u{b4}'\n'\u{b5}'\n'\u{b6}'\n'\u{b7}'\n'\u{b8}'\n'\u{b9}'\n'\u{ba}'\n'\u{bb}'\n'\u{bc}'\n'\u{bd}'\n'\u{be}'\n'\u{bf}'\n'\u{c0}'\n'\u{c1}'\n'\u{c2}'\n'\u{c3}'\n'\u{c4}'\n'\u{c5}'\n'\u{c6}'\n'\u{c7}'\n'\u{c8}'\n'\u{c9}'\n'\u{ca}'\n'\u{cb}'\n'\u{cc}'\n'\u{cd}'\n'\u{ce}'\n'\u{cf}'\n'\u{d0}'\n'\u{d1}'\n'\u{d2}'\n'\u{d3}'\n'\u{d4}'\n'\u{d5}'\n'\u{d6}'\n'\u{d7}'\n'\u{d8}'\n'\u{d9}'\n'\u{da}'\n'\u{db}'\n'\u{dc}'\n'\u{dd}'\n'\u{de}'\n'\u{df}'\n'\u{e0}'\n'\u{e1}'\n'\u{e2}'\n'\u{e3}'\n'\u{e4}'\n'\u{e5}'\n'\u{e6}'\n'\u{e7}'\n'\u{e8}'\n'\u{e9}'\n'\u{ea}'\n'\u{eb}'\n'\u{ec}'\n'\u{ed}'\n'\u{ee}'\n'\u{ef}'\n'\u{f0}'\n'\u{f1}'\n'\u{f2}'\n'\u{f3}'\n'\u{f4}'\n'\u{f5}'\n'\u{f6}'\n'\u{f7}'\n'\u{f8}'\n'\u{f9}'\n'\u{fa}'\n'\u{fb}'\n'\u{fc}'\n'\u{fd}'\n'\u{fe}'\n'\u{ff}'",
            ),
            (
                "isspace",
                "0000000001111100000000000000111110000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "isdigit",
                "0000000000000000000000000000000000000000000000001111111111000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000110000010000000000000000000000000000000000000000000000000000000000000000000000",
            ),
            (
                "isalpha",
                "0000000000000000000000000000000000000000000000000000000000000000011111111111111111111111111000000111111111111111111111111110000000000000000000000000000000000000000000000010000000000100001000001111111111111111111111101111111111111111111111111111111011111111",
            ),
            (
                "isalnum",
                "0000000000000000000000000000000000000000000000001111111111000000011111111111111111111111111000000111111111111111111111111110000000000000000000000000000000000000000000000010000000110100011011101111111111111111111111101111111111111111111111111111111011111111",
            ),
        ];
        for (name, expected) in tables {
            let got = match name {
                "upper" => per_char(upper),
                "lower" => per_char(lower),
                "repr" => per_char(repr_str),
                "isspace" => bits(isspace),
                "isdigit" => bits(isdigit),
                "isalpha" => bits(isalpha),
                _ => bits(isalnum),
            };
            assert_eq!(got, expected, "{name}");
        }
    }

    #[test]
    fn int_division() {
        let rows: &[(i64, i64, Option<i64>, Option<i64>)] = &[
            (7, 2, Some(3), Some(1)),
            (7, -2, Some(-4), Some(-1)),
            (7, 3, Some(2), Some(1)),
            (7, -3, Some(-3), Some(-2)),
            (7, 1, Some(7), Some(0)),
            (7, -1, Some(-7), Some(0)),
            (7, 7, Some(1), Some(0)),
            (7, -7, Some(-1), Some(0)),
            (7, 0, None, None),
            (7, 9223372036854775807, Some(0), Some(7)),
            (-7, 2, Some(-4), Some(1)),
            (-7, -2, Some(3), Some(-1)),
            (-7, 3, Some(-3), Some(2)),
            (-7, -3, Some(2), Some(-1)),
            (-7, 1, Some(-7), Some(0)),
            (-7, -1, Some(7), Some(0)),
            (-7, 7, Some(-1), Some(0)),
            (-7, -7, Some(1), Some(0)),
            (-7, 0, None, None),
            (-7, 9223372036854775807, Some(-1), Some(9223372036854775800)),
            (6, 2, Some(3), Some(0)),
            (6, -2, Some(-3), Some(0)),
            (6, 3, Some(2), Some(0)),
            (6, -3, Some(-2), Some(0)),
            (6, 1, Some(6), Some(0)),
            (6, -1, Some(-6), Some(0)),
            (6, 7, Some(0), Some(6)),
            (6, -7, Some(-1), Some(-1)),
            (6, 0, None, None),
            (6, 9223372036854775807, Some(0), Some(6)),
            (-6, 2, Some(-3), Some(0)),
            (-6, -2, Some(3), Some(0)),
            (-6, 3, Some(-2), Some(0)),
            (-6, -3, Some(2), Some(0)),
            (-6, 1, Some(-6), Some(0)),
            (-6, -1, Some(6), Some(0)),
            (-6, 7, Some(-1), Some(1)),
            (-6, -7, Some(0), Some(-6)),
            (-6, 0, None, None),
            (-6, 9223372036854775807, Some(-1), Some(9223372036854775801)),
            (0, 2, Some(0), Some(0)),
            (0, -2, Some(0), Some(0)),
            (0, 3, Some(0), Some(0)),
            (0, -3, Some(0), Some(0)),
            (0, 1, Some(0), Some(0)),
            (0, -1, Some(0), Some(0)),
            (0, 7, Some(0), Some(0)),
            (0, -7, Some(0), Some(0)),
            (0, 0, None, None),
            (0, 9223372036854775807, Some(0), Some(0)),
            (1, 2, Some(0), Some(1)),
            (1, -2, Some(-1), Some(-1)),
            (1, 3, Some(0), Some(1)),
            (1, -3, Some(-1), Some(-2)),
            (1, 1, Some(1), Some(0)),
            (1, -1, Some(-1), Some(0)),
            (1, 7, Some(0), Some(1)),
            (1, -7, Some(-1), Some(-6)),
            (1, 0, None, None),
            (1, 9223372036854775807, Some(0), Some(1)),
            (-1, 2, Some(-1), Some(1)),
            (-1, -2, Some(0), Some(-1)),
            (-1, 3, Some(-1), Some(2)),
            (-1, -3, Some(0), Some(-1)),
            (-1, 1, Some(-1), Some(0)),
            (-1, -1, Some(1), Some(0)),
            (-1, 7, Some(-1), Some(6)),
            (-1, -7, Some(0), Some(-1)),
            (-1, 0, None, None),
            (-1, 9223372036854775807, Some(-1), Some(9223372036854775806)),
            (4611686018427387904, 2, Some(2305843009213693952), Some(0)),
            (4611686018427387904, -2, Some(-2305843009213693952), Some(0)),
            (4611686018427387904, 3, Some(1537228672809129301), Some(1)),
            (
                4611686018427387904,
                -3,
                Some(-1537228672809129302),
                Some(-2),
            ),
            (4611686018427387904, 1, Some(4611686018427387904), Some(0)),
            (4611686018427387904, -1, Some(-4611686018427387904), Some(0)),
            (4611686018427387904, 7, Some(658812288346769700), Some(4)),
            (4611686018427387904, -7, Some(-658812288346769701), Some(-3)),
            (4611686018427387904, 0, None, None),
            (
                4611686018427387904,
                9223372036854775807,
                Some(0),
                Some(4611686018427387904),
            ),
            (-4611686018427387904, 2, Some(-2305843009213693952), Some(0)),
            (-4611686018427387904, -2, Some(2305843009213693952), Some(0)),
            (-4611686018427387904, 3, Some(-1537228672809129302), Some(2)),
            (
                -4611686018427387904,
                -3,
                Some(1537228672809129301),
                Some(-1),
            ),
            (-4611686018427387904, 1, Some(-4611686018427387904), Some(0)),
            (-4611686018427387904, -1, Some(4611686018427387904), Some(0)),
            (-4611686018427387904, 7, Some(-658812288346769701), Some(3)),
            (-4611686018427387904, -7, Some(658812288346769700), Some(-4)),
            (-4611686018427387904, 0, None, None),
            (
                -4611686018427387904,
                9223372036854775807,
                Some(-1),
                Some(4611686018427387903),
            ),
            (9223372036854775807, 2, Some(4611686018427387903), Some(1)),
            (
                9223372036854775807,
                -2,
                Some(-4611686018427387904),
                Some(-1),
            ),
            (9223372036854775807, 3, Some(3074457345618258602), Some(1)),
            (
                9223372036854775807,
                -3,
                Some(-3074457345618258603),
                Some(-2),
            ),
            (9223372036854775807, 1, Some(9223372036854775807), Some(0)),
            (9223372036854775807, -1, Some(-9223372036854775807), Some(0)),
            (9223372036854775807, 7, Some(1317624576693539401), Some(0)),
            (9223372036854775807, -7, Some(-1317624576693539401), Some(0)),
            (9223372036854775807, 0, None, None),
            (9223372036854775807, 9223372036854775807, Some(1), Some(0)),
            (i64::MIN, 2, Some(-4611686018427387904), Some(0)),
            (i64::MIN, -2, Some(4611686018427387904), Some(0)),
            (i64::MIN, 3, Some(-3074457345618258603), Some(1)),
            (i64::MIN, -3, Some(3074457345618258602), Some(-2)),
            (i64::MIN, 1, Some(-9223372036854775808), Some(0)),
            (i64::MIN, 7, Some(-1317624576693539402), Some(6)),
            (i64::MIN, -7, Some(1317624576693539401), Some(-1)),
            (i64::MIN, 0, None, None),
            (
                i64::MIN,
                9223372036854775807,
                Some(-2),
                Some(9223372036854775806),
            ),
        ];
        for &(a, b, q, r) in rows {
            assert_eq!(floordiv_i64(a, b), q, "{a} // {b}");
            assert_eq!(pymod_i64(a, b), r, "{a} % {b}");
            assert_eq!(divmod_i64(a, b), q.zip(r), "divmod({a}, {b})");
        }
        assert_eq!(floordiv_i64(i64::MIN, -1), None);
    }

    #[test]
    fn float_division() {
        let rows: &[(f64, f64, Option<f64>, Option<f64>)] = &[
            (7.5_f64, 2.0_f64, Some(3.0_f64), Some(1.5_f64)),
            (7.5_f64, -2.0_f64, Some(-4.0_f64), Some(-0.5_f64)),
            (
                7.5_f64,
                0.1_f64,
                Some(74.0_f64),
                Some(0.09999999999999959_f64),
            ),
            (
                7.5_f64,
                -0.1_f64,
                Some(-75.0_f64),
                Some(-4.163336342344337e-16_f64),
            ),
            (7.5_f64, 0.0_f64, None, None),
            (7.5_f64, f64::INFINITY, Some(0.0_f64), Some(7.5_f64)),
            (
                7.5_f64,
                f64::NEG_INFINITY,
                Some(-1.0_f64),
                Some(f64::NEG_INFINITY),
            ),
            (7.5_f64, 0.5_f64, Some(15.0_f64), Some(0.0_f64)),
            (
                7.5_f64,
                1e-300_f64,
                Some(7.5e+300_f64),
                Some(7.223898960297836e-301_f64),
            ),
            (7.5_f64, -3.0_f64, Some(-3.0_f64), Some(-1.5_f64)),
            (-7.5_f64, 2.0_f64, Some(-4.0_f64), Some(0.5_f64)),
            (-7.5_f64, -2.0_f64, Some(3.0_f64), Some(-1.5_f64)),
            (
                -7.5_f64,
                0.1_f64,
                Some(-75.0_f64),
                Some(4.163336342344337e-16_f64),
            ),
            (
                -7.5_f64,
                -0.1_f64,
                Some(74.0_f64),
                Some(-0.09999999999999959_f64),
            ),
            (-7.5_f64, 0.0_f64, None, None),
            (-7.5_f64, f64::INFINITY, Some(-1.0_f64), Some(f64::INFINITY)),
            (-7.5_f64, f64::NEG_INFINITY, Some(0.0_f64), Some(-7.5_f64)),
            (-7.5_f64, 0.5_f64, Some(-15.0_f64), Some(0.0_f64)),
            (
                -7.5_f64,
                1e-300_f64,
                Some(-7.5e+300_f64),
                Some(2.776101039702164e-301_f64),
            ),
            (-7.5_f64, -3.0_f64, Some(2.0_f64), Some(-1.5_f64)),
            (6.0_f64, 2.0_f64, Some(3.0_f64), Some(0.0_f64)),
            (6.0_f64, -2.0_f64, Some(-3.0_f64), Some(-0.0_f64)),
            (
                6.0_f64,
                0.1_f64,
                Some(59.0_f64),
                Some(0.09999999999999967_f64),
            ),
            (
                6.0_f64,
                -0.1_f64,
                Some(-60.0_f64),
                Some(-3.3306690738754696e-16_f64),
            ),
            (6.0_f64, 0.0_f64, None, None),
            (6.0_f64, f64::INFINITY, Some(0.0_f64), Some(6.0_f64)),
            (
                6.0_f64,
                f64::NEG_INFINITY,
                Some(-1.0_f64),
                Some(f64::NEG_INFINITY),
            ),
            (6.0_f64, 0.5_f64, Some(12.0_f64), Some(0.0_f64)),
            (
                6.0_f64,
                1e-300_f64,
                Some(6e+300_f64),
                Some(3.779119168238269e-301_f64),
            ),
            (6.0_f64, -3.0_f64, Some(-2.0_f64), Some(-0.0_f64)),
            (-6.0_f64, 2.0_f64, Some(-3.0_f64), Some(0.0_f64)),
            (-6.0_f64, -2.0_f64, Some(3.0_f64), Some(-0.0_f64)),
            (
                -6.0_f64,
                0.1_f64,
                Some(-60.0_f64),
                Some(3.3306690738754696e-16_f64),
            ),
            (
                -6.0_f64,
                -0.1_f64,
                Some(59.0_f64),
                Some(-0.09999999999999967_f64),
            ),
            (-6.0_f64, 0.0_f64, None, None),
            (-6.0_f64, f64::INFINITY, Some(-1.0_f64), Some(f64::INFINITY)),
            (-6.0_f64, f64::NEG_INFINITY, Some(0.0_f64), Some(-6.0_f64)),
            (-6.0_f64, 0.5_f64, Some(-12.0_f64), Some(0.0_f64)),
            (
                -6.0_f64,
                1e-300_f64,
                Some(-6e+300_f64),
                Some(6.220880831761731e-301_f64),
            ),
            (-6.0_f64, -3.0_f64, Some(2.0_f64), Some(-0.0_f64)),
            (0.0_f64, 2.0_f64, Some(0.0_f64), Some(0.0_f64)),
            (0.0_f64, -2.0_f64, Some(-0.0_f64), Some(-0.0_f64)),
            (0.0_f64, 0.1_f64, Some(0.0_f64), Some(0.0_f64)),
            (0.0_f64, -0.1_f64, Some(-0.0_f64), Some(-0.0_f64)),
            (0.0_f64, 0.0_f64, None, None),
            (0.0_f64, f64::INFINITY, Some(0.0_f64), Some(0.0_f64)),
            (0.0_f64, f64::NEG_INFINITY, Some(-0.0_f64), Some(-0.0_f64)),
            (0.0_f64, 0.5_f64, Some(0.0_f64), Some(0.0_f64)),
            (0.0_f64, 1e-300_f64, Some(0.0_f64), Some(0.0_f64)),
            (0.0_f64, -3.0_f64, Some(-0.0_f64), Some(-0.0_f64)),
            (-0.0_f64, 2.0_f64, Some(-0.0_f64), Some(0.0_f64)),
            (-0.0_f64, -2.0_f64, Some(0.0_f64), Some(-0.0_f64)),
            (-0.0_f64, 0.1_f64, Some(-0.0_f64), Some(0.0_f64)),
            (-0.0_f64, -0.1_f64, Some(0.0_f64), Some(-0.0_f64)),
            (-0.0_f64, 0.0_f64, None, None),
            (-0.0_f64, f64::INFINITY, Some(-0.0_f64), Some(0.0_f64)),
            (-0.0_f64, f64::NEG_INFINITY, Some(0.0_f64), Some(-0.0_f64)),
            (-0.0_f64, 0.5_f64, Some(-0.0_f64), Some(0.0_f64)),
            (-0.0_f64, 1e-300_f64, Some(-0.0_f64), Some(0.0_f64)),
            (-0.0_f64, -3.0_f64, Some(0.0_f64), Some(-0.0_f64)),
            (0.3_f64, 2.0_f64, Some(0.0_f64), Some(0.3_f64)),
            (0.3_f64, -2.0_f64, Some(-1.0_f64), Some(-1.7_f64)),
            (
                0.3_f64,
                0.1_f64,
                Some(2.0_f64),
                Some(0.09999999999999998_f64),
            ),
            (
                0.3_f64,
                -0.1_f64,
                Some(-3.0_f64),
                Some(-2.7755575615628914e-17_f64),
            ),
            (0.3_f64, 0.0_f64, None, None),
            (0.3_f64, f64::INFINITY, Some(0.0_f64), Some(0.3_f64)),
            (
                0.3_f64,
                f64::NEG_INFINITY,
                Some(-1.0_f64),
                Some(f64::NEG_INFINITY),
            ),
            (0.3_f64, 0.5_f64, Some(0.0_f64), Some(0.3_f64)),
            (
                0.3_f64,
                1e-300_f64,
                Some(2.9999999999999998e+299_f64),
                Some(1.7742849249661237e-301_f64),
            ),
            (0.3_f64, -3.0_f64, Some(-1.0_f64), Some(-2.7_f64)),
            (1e+300_f64, 2.0_f64, Some(5e+299_f64), Some(0.0_f64)),
            (1e+300_f64, -2.0_f64, Some(-5e+299_f64), Some(-0.0_f64)),
            (
                1e+300_f64,
                0.1_f64,
                Some(1e+301_f64),
                Some(0.00011215964963492975_f64),
            ),
            (
                1e+300_f64,
                -0.1_f64,
                Some(-1e+301_f64),
                Some(-0.09988784035036508_f64),
            ),
            (1e+300_f64, 0.0_f64, None, None),
            (1e+300_f64, f64::INFINITY, Some(0.0_f64), Some(1e+300_f64)),
            (
                1e+300_f64,
                f64::NEG_INFINITY,
                Some(-1.0_f64),
                Some(f64::NEG_INFINITY),
            ),
            (1e+300_f64, 0.5_f64, Some(2e+300_f64), Some(0.0_f64)),
            (
                1e+300_f64,
                1e-300_f64,
                Some(f64::INFINITY),
                Some(4.891554850853602e-301_f64),
            ),
            (
                1e+300_f64,
                -3.0_f64,
                Some(-3.3333333333333335e+299_f64),
                Some(-0.0_f64),
            ),
            (f64::INFINITY, 2.0_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::INFINITY, -2.0_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::INFINITY, 0.1_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::INFINITY, -0.1_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::INFINITY, 0.0_f64, None, None),
            (f64::INFINITY, f64::INFINITY, Some(f64::NAN), Some(f64::NAN)),
            (
                f64::INFINITY,
                f64::NEG_INFINITY,
                Some(f64::NAN),
                Some(f64::NAN),
            ),
            (f64::INFINITY, 0.5_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::INFINITY, 1e-300_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::INFINITY, -3.0_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, 2.0_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, -2.0_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, 0.1_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, -0.1_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, 0.0_f64, None, None),
            (f64::NAN, f64::INFINITY, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, f64::NEG_INFINITY, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, 0.5_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, 1e-300_f64, Some(f64::NAN), Some(f64::NAN)),
            (f64::NAN, -3.0_f64, Some(f64::NAN), Some(f64::NAN)),
            (5.0_f64, 2.0_f64, Some(2.0_f64), Some(1.0_f64)),
            (5.0_f64, -2.0_f64, Some(-3.0_f64), Some(-1.0_f64)),
            (
                5.0_f64,
                0.1_f64,
                Some(49.0_f64),
                Some(0.09999999999999973_f64),
            ),
            (
                5.0_f64,
                -0.1_f64,
                Some(-50.0_f64),
                Some(-2.7755575615628914e-16_f64),
            ),
            (5.0_f64, 0.0_f64, None, None),
            (5.0_f64, f64::INFINITY, Some(0.0_f64), Some(5.0_f64)),
            (
                5.0_f64,
                f64::NEG_INFINITY,
                Some(-1.0_f64),
                Some(f64::NEG_INFINITY),
            ),
            (5.0_f64, 0.5_f64, Some(10.0_f64), Some(0.0_f64)),
            (
                5.0_f64,
                1e-300_f64,
                Some(4.9999999999999997e+300_f64),
                Some(4.8159326401985574e-301_f64),
            ),
            (5.0_f64, -3.0_f64, Some(-2.0_f64), Some(-1.0_f64)),
            (-0.5_f64, 2.0_f64, Some(-1.0_f64), Some(1.5_f64)),
            (-0.5_f64, -2.0_f64, Some(0.0_f64), Some(-0.5_f64)),
            (
                -0.5_f64,
                0.1_f64,
                Some(-5.0_f64),
                Some(2.7755575615628914e-17_f64),
            ),
            (
                -0.5_f64,
                -0.1_f64,
                Some(4.0_f64),
                Some(-0.09999999999999998_f64),
            ),
            (-0.5_f64, 0.0_f64, None, None),
            (-0.5_f64, f64::INFINITY, Some(-1.0_f64), Some(f64::INFINITY)),
            (-0.5_f64, f64::NEG_INFINITY, Some(0.0_f64), Some(-0.5_f64)),
            (-0.5_f64, 0.5_f64, Some(-1.0_f64), Some(0.0_f64)),
            (
                -0.5_f64,
                1e-300_f64,
                Some(-4.9999999999999995e+299_f64),
                Some(5.518406735980144e-301_f64),
            ),
            (-0.5_f64, -3.0_f64, Some(0.0_f64), Some(-0.5_f64)),
        ];
        for &(a, b, q, r) in rows {
            assert!(
                feq(floordiv_f64(a, b), q),
                "{a} // {b} = {:?}",
                floordiv_f64(a, b)
            );
            assert!(feq(pymod_f64(a, b), r), "{a} % {b} = {:?}", pymod_f64(a, b));
        }
    }

    #[test]
    fn int_round_and_is_integer() {
        let rounds: &[(f64, Option<i64>)] = &[
            (0.5_f64, Some(0)),
            (1.5_f64, Some(2)),
            (2.5_f64, Some(2)),
            (-0.5_f64, Some(0)),
            (-1.5_f64, Some(-2)),
            (-2.5_f64, Some(-2)),
            (0.49999999999999994_f64, Some(0)),
            (2.675_f64, Some(3)),
            (1000000000000000.5_f64, Some(1000000000000000)),
            (1234567.5_f64, Some(1234568)),
            (-0.0_f64, Some(0)),
            (1e+18_f64, Some(1000000000000000000)),
            (3.7_f64, Some(4)),
            (-3.7_f64, Some(-4)),
            (f64::NAN, None),
            (f64::INFINITY, None),
            (f64::NEG_INFINITY, None),
            (0.0_f64, Some(0)),
            (100.5_f64, Some(100)),
            (101.5_f64, Some(102)),
        ];
        for &(x, e) in rounds {
            assert_eq!(py_round(x), e, "round({x})");
        }
        let ints: &[(f64, Option<i64>)] = &[
            (0.5_f64, Some(0)),
            (1.5_f64, Some(1)),
            (2.5_f64, Some(2)),
            (-0.5_f64, Some(0)),
            (-1.5_f64, Some(-1)),
            (-2.5_f64, Some(-2)),
            (0.49999999999999994_f64, Some(0)),
            (2.675_f64, Some(2)),
            (1000000000000000.5_f64, Some(1000000000000000)),
            (1234567.5_f64, Some(1234567)),
            (-0.0_f64, Some(0)),
            (1e+18_f64, Some(1000000000000000000)),
            (3.7_f64, Some(3)),
            (-3.7_f64, Some(-3)),
            (f64::NAN, None),
            (f64::INFINITY, None),
            (f64::NEG_INFINITY, None),
            (0.0_f64, Some(0)),
            (100.5_f64, Some(100)),
            (101.5_f64, Some(101)),
        ];
        for &(x, e) in ints {
            assert_eq!(py_int_from_f64(x), e, "int({x})");
        }
        let isint: &[(f64, bool)] = &[
            (1.0_f64, true),
            (1.5_f64, false),
            (-0.0_f64, true),
            (1e+300_f64, true),
            (f64::INFINITY, false),
            (f64::NAN, false),
            (0.30000000000000004_f64, false),
            (3.0_f64, true),
        ];
        for &(x, e) in isint {
            assert_eq!(is_integer(x), e, "{x}.is_integer()");
        }
    }

    #[test]
    fn parse_numbers() {
        let ints: &[(&str, Option<i64>)] = &[
            ("12", Some(12)),
            (" 12 ", Some(12)),
            ("+5", Some(5)),
            ("-5", Some(-5)),
            ("1_000", Some(1000)),
            ("1__0", None),
            ("_1", None),
            ("1_", None),
            ("", None),
            (" ", None),
            ("+", None),
            ("-", None),
            ("0x10", None),
            ("1.0", None),
            ("\u{a0}12\u{85}", Some(12)),
            ("12\n", Some(12)),
            ("007", Some(7)),
            ("9223372036854775807", Some(9223372036854775807)),
            ("- 5", None),
            ("--5", None),
            ("1 2", None),
            ("\t-7\r\n", Some(-7)),
            ("0_0", Some(0)),
            ("+_1", None),
            ("-0", Some(0)),
            ("1e3", None),
            ("12a", None),
            ("\u{1c}5\u{1f}", None),
            ("\u{1c}5", None),
            ("-9223372036854775808", Some(-9223372036854775808)),
        ];
        for &(s, e) in ints {
            assert_eq!(parse_int(s), e, "int({s:?})");
        }
        let radix: &[(&str, u32, Option<i64>)] = &[
            ("ff", 16, Some(255)),
            ("0xFF", 16, Some(255)),
            ("0x_ff", 16, Some(255)),
            ("FF", 16, Some(255)),
            ("-0x1f", 16, Some(-31)),
            ("101", 2, Some(5)),
            ("0b101", 2, Some(5)),
            ("12", 2, None),
            ("0x", 16, None),
            ("", 16, None),
            ("0X1f", 16, Some(31)),
            ("g", 16, None),
            ("&H", 16, None),
            ("0b", 2, None),
            ("1_0", 2, Some(2)),
            ("_1", 16, None),
            ("0xx1", 16, None),
            (" ff ", 16, Some(255)),
            ("0x0x1", 16, None),
            ("0o17", 8, Some(15)),
            ("777", 8, Some(511)),
            ("00ff", 16, Some(255)),
        ];
        for &(s, b, e) in radix {
            assert_eq!(parse_int_radix(s, b), e, "int({s:?}, {b})");
        }
        let floats: &[(&str, Option<f64>)] = &[
            ("1", Some(1.0_f64)),
            ("1.5", Some(1.5_f64)),
            (".5", Some(0.5_f64)),
            ("5.", Some(5.0_f64)),
            ("-.5e3", Some(-500.0_f64)),
            ("1e5", Some(100000.0_f64)),
            ("1E-2", Some(0.01_f64)),
            ("1e", None),
            ("e5", None),
            (".", None),
            ("+", None),
            ("inf", Some(f64::INFINITY)),
            ("-Infinity", Some(f64::NEG_INFINITY)),
            ("NaN", Some(f64::NAN)),
            ("-nan", Some(f64::NAN)),
            ("infinit", None),
            (" 1.5 ", Some(1.5_f64)),
            ("1_0.5", Some(10.5_f64)),
            ("1._5", None),
            ("1_.5", None),
            ("1e1_0", Some(10000000000.0_f64)),
            ("0x10", None),
            ("1e500", Some(f64::INFINITY)),
            ("-1e500", Some(f64::NEG_INFINITY)),
            ("1,5", None),
            ("", None),
            ("+-1", None),
            ("1 2", None),
            ("1e+", None),
            ("1.e3", Some(1000.0_f64)),
            ("1e5.5", None),
            ("1e-3", Some(0.001_f64)),
            ("+.5", Some(0.5_f64)),
            ("-0", Some(-0.0_f64)),
            ("-0.0", Some(-0.0_f64)),
            ("0.1", Some(0.1_f64)),
            (
                "123456789012345678901234567890",
                Some(1.2345678901234568e+29_f64),
            ),
            ("1e-400", Some(0.0_f64)),
            ("\u{a0}1.5\u{85}", Some(1.5_f64)),
            ("in f", None),
            ("+inf", Some(f64::INFINITY)),
            ("1__0", None),
            ("1e_5", None),
            ("1_e5", None),
            ("00.5", Some(0.5_f64)),
            ("5e+0_1", Some(50.0_f64)),
            ("nan1", None),
            ("\u{1c}1.5\u{1f}", None),
            ("1.5\u{1f}", None),
            ("\u{b}1.5\u{c}", Some(1.5_f64)),
            ("infinity ", Some(f64::INFINITY)),
            ("+nan", Some(f64::NAN)),
            (".e5", None),
            ("1.5.5", None),
            ("0_1.0_1", Some(1.01_f64)),
        ];
        for &(s, e) in floats {
            assert!(
                feq(parse_float(s), e),
                "float({s:?}) = {:?}",
                parse_float(s)
            );
        }
    }

    #[test]
    fn float_formatting() {
        let reprs: &[(f64, &str)] = &[
            (0.30000000000000004_f64, "0.30000000000000004"),
            (1e+16_f64, "1e+16"),
            (1000000000000000.0_f64, "1000000000000000.0"),
            (9999999999999998.0_f64, "9999999999999998.0"),
            (1.0_f64, "1.0"),
            (100.0_f64, "100.0"),
            (1e+22_f64, "1e+22"),
            (1e-05_f64, "1e-05"),
            (0.0001_f64, "0.0001"),
            (0.00012345_f64, "0.00012345"),
            (1.2345678901234568e+17_f64, "1.2345678901234568e+17"),
            (5e-324_f64, "5e-324"),
            (1.7976931348623157e+308_f64, "1.7976931348623157e+308"),
            (-0.0_f64, "-0.0"),
            (0.0_f64, "0.0"),
            (2.5_f64, "2.5"),
            (0.3333333333333333_f64, "0.3333333333333333"),
            (1e+100_f64, "1e+100"),
            (123456.789_f64, "123456.789"),
            (1e-07_f64, "1e-07"),
            (1.2345678901234567e+19_f64, "1.2345678901234567e+19"),
            (1.5e+300_f64, "1.5e+300"),
            (-1e-10_f64, "-1e-10"),
            (3.0_f64, "3.0"),
            (255.0_f64, "255.0"),
            (0.5_f64, "0.5"),
            (4.35_f64, "4.35"),
            (1e+21_f64, "1e+21"),
            (123456789.12345679_f64, "123456789.12345679"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
            (9007199254740992.0_f64, "9007199254740992.0"),
            (1e-06_f64, "1e-06"),
            (1.1e-05_f64, "1.1e-05"),
            (-1.5_f64, "-1.5"),
            (1234567890123456.0_f64, "1234567890123456.0"),
            (1.2345678901234568e+16_f64, "1.2345678901234568e+16"),
            (0.1_f64, "0.1"),
            (0.0001_f64, "0.0001"),
            (0.00099_f64, "0.00099"),
            (1e+17_f64, "1e+17"),
            (5000000000000000.0_f64, "5000000000000000.0"),
        ];
        for &(x, e) in reprs {
            assert_eq!(repr_f64(x), e, "repr({x:?})");
        }
        let g: &[(f64, usize, &str)] = &[
            (100000.0_f64, 6, "100000"),
            (100000.0_f64, 1, "1e+05"),
            (100000.0_f64, 3, "1e+05"),
            (100000.0_f64, 10, "100000"),
            (100000.0_f64, 0, "1e+05"),
            (1000000.0_f64, 6, "1e+06"),
            (0.0001_f64, 6, "0.0001"),
            (1e-05_f64, 6, "1e-05"),
            (1e-05_f64, 1, "1e-05"),
            (1e-05_f64, 3, "1e-05"),
            (1e-05_f64, 10, "1e-05"),
            (1e-05_f64, 0, "1e-05"),
            (0.1_f64, 6, "0.1"),
            (123456789.0_f64, 6, "1.23457e+08"),
            (0.5_f64, 6, "0.5"),
            (3.0_f64, 6, "3"),
            (1234567.0_f64, 6, "1.23457e+06"),
            (1e+16_f64, 6, "1e+16"),
            (1e+16_f64, 1, "1e+16"),
            (1e+16_f64, 3, "1e+16"),
            (1e+16_f64, 10, "1e+16"),
            (1e+16_f64, 0, "1e+16"),
            (2.5e-10_f64, 6, "2.5e-10"),
            (0.000123456789_f64, 6, "0.000123457"),
            (999999.5_f64, 6, "1e+06"),
            (999999.5_f64, 1, "1e+06"),
            (999999.5_f64, 3, "1e+06"),
            (999999.5_f64, 10, "999999.5"),
            (999999.5_f64, 0, "1e+06"),
            (9.9999995_f64, 6, "10"),
            (9.9999995_f64, 1, "1e+01"),
            (9.9999995_f64, 3, "10"),
            (9.9999995_f64, 10, "9.9999995"),
            (9.9999995_f64, 0, "1e+01"),
            (-1.5_f64, 6, "-1.5"),
            (0.0_f64, 6, "0"),
            (-0.0_f64, 6, "-0"),
            (1e+100_f64, 6, "1e+100"),
            (f64::INFINITY, 6, "inf"),
            (f64::NAN, 6, "nan"),
            (100000.0_f64, 6, "100000"),
            (100000.0_f64, 1, "1e+05"),
            (100000.0_f64, 3, "1e+05"),
            (100000.0_f64, 10, "100000"),
            (100000.0_f64, 0, "1e+05"),
            (123456.0_f64, 6, "123456"),
            (0.0001234_f64, 6, "0.0001234"),
            (0.0001234_f64, 1, "0.0001"),
            (0.0001234_f64, 3, "0.000123"),
            (0.0001234_f64, 10, "0.0001234"),
            (0.0001234_f64, 0, "0.0001"),
            (5e-324_f64, 6, "4.94066e-324"),
            (1.0_f64, 6, "1"),
            (12.0_f64, 6, "12"),
            (100.0_f64, 6, "100"),
            (2.675_f64, 6, "2.675"),
            (2.675_f64, 1, "3"),
            (2.675_f64, 3, "2.67"),
            (2.675_f64, 10, "2.675"),
            (2.675_f64, 0, "3"),
            (-1.234e-05_f64, 6, "-1.234e-05"),
            (99999.95_f64, 6, "99999.9"),
            (99999.95_f64, 1, "1e+05"),
            (99999.95_f64, 3, "1e+05"),
            (99999.95_f64, 10, "99999.95"),
            (99999.95_f64, 0, "1e+05"),
            (0.30000000000000004_f64, 6, "0.3"),
            (1000000000000000.0_f64, 6, "1e+15"),
            (4.35_f64, 6, "4.35"),
            (7.0_f64, 6, "7"),
            (31.5_f64, 6, "31.5"),
            (1e-300_f64, 6, "1e-300"),
            (1234.5678_f64, 6, "1234.57"),
            (1234.5678_f64, 1, "1e+03"),
            (1234.5678_f64, 3, "1.23e+03"),
            (1234.5678_f64, 10, "1234.5678"),
            (1234.5678_f64, 0, "1e+03"),
        ];
        for &(x, p, e) in g {
            assert_eq!(format_g(x, p), e, "format({x:?}, '.{p}g')");
        }
        let es: &[(f64, usize, &str)] = &[
            (0.0_f64, 4, "0.0000e+00"),
            (0.0_f64, 0, "0e+00"),
            (0.0_f64, 1, "0.0e+00"),
            (0.0_f64, 10, "0.0000000000e+00"),
            (-0.0_f64, 4, "-0.0000e+00"),
            (-0.0_f64, 0, "-0e+00"),
            (-0.0_f64, 1, "-0.0e+00"),
            (-0.0_f64, 10, "-0.0000000000e+00"),
            (1.0_f64, 4, "1.0000e+00"),
            (1.0_f64, 0, "1e+00"),
            (1.0_f64, 1, "1.0e+00"),
            (1.0_f64, 10, "1.0000000000e+00"),
            (12345.6789_f64, 4, "1.2346e+04"),
            (12345.6789_f64, 0, "1e+04"),
            (12345.6789_f64, 1, "1.2e+04"),
            (12345.6789_f64, 10, "1.2345678900e+04"),
            (0.000123456_f64, 4, "1.2346e-04"),
            (0.000123456_f64, 0, "1e-04"),
            (0.000123456_f64, 1, "1.2e-04"),
            (0.000123456_f64, 10, "1.2345600000e-04"),
            (1e+100_f64, 4, "1.0000e+100"),
            (1e+100_f64, 0, "1e+100"),
            (1e+100_f64, 1, "1.0e+100"),
            (1e+100_f64, 10, "1.0000000000e+100"),
            (9.99995_f64, 4, "1.0000e+01"),
            (9.99995_f64, 0, "1e+01"),
            (9.99995_f64, 1, "1.0e+01"),
            (9.99995_f64, 10, "9.9999500000e+00"),
            (1e-300_f64, 4, "1.0000e-300"),
            (1e-300_f64, 0, "1e-300"),
            (1e-300_f64, 1, "1.0e-300"),
            (1e-300_f64, 10, "1.0000000000e-300"),
            (5e-324_f64, 4, "4.9407e-324"),
            (5e-324_f64, 0, "5e-324"),
            (5e-324_f64, 1, "4.9e-324"),
            (5e-324_f64, 10, "4.9406564584e-324"),
            (1.5_f64, 4, "1.5000e+00"),
            (1.5_f64, 0, "2e+00"),
            (1.5_f64, 1, "1.5e+00"),
            (1.5_f64, 10, "1.5000000000e+00"),
            (2.5_f64, 4, "2.5000e+00"),
            (2.5_f64, 0, "2e+00"),
            (2.5_f64, 1, "2.5e+00"),
            (2.5_f64, 10, "2.5000000000e+00"),
            (f64::INFINITY, 4, "inf"),
            (f64::INFINITY, 0, "inf"),
            (f64::INFINITY, 1, "inf"),
            (f64::INFINITY, 10, "inf"),
            (f64::NAN, 4, "nan"),
            (f64::NAN, 0, "nan"),
            (f64::NAN, 1, "nan"),
            (f64::NAN, 10, "nan"),
            (-314159.0_f64, 4, "-3.1416e+05"),
            (-314159.0_f64, 0, "-3e+05"),
            (-314159.0_f64, 1, "-3.1e+05"),
            (-314159.0_f64, 10, "-3.1415900000e+05"),
            (1e+16_f64, 4, "1.0000e+16"),
            (1e+16_f64, 0, "1e+16"),
            (1e+16_f64, 1, "1.0e+16"),
            (1e+16_f64, 10, "1.0000000000e+16"),
            (0.5_f64, 4, "5.0000e-01"),
            (0.5_f64, 0, "5e-01"),
            (0.5_f64, 1, "5.0e-01"),
            (0.5_f64, 10, "5.0000000000e-01"),
            (1e+22_f64, 4, "1.0000e+22"),
            (1e+22_f64, 0, "1e+22"),
            (1e+22_f64, 1, "1.0e+22"),
            (1e+22_f64, 10, "1.0000000000e+22"),
            (99999.5_f64, 4, "1.0000e+05"),
            (99999.5_f64, 0, "1e+05"),
            (99999.5_f64, 1, "1.0e+05"),
            (99999.5_f64, 10, "9.9999500000e+04"),
            (0.30000000000000004_f64, 4, "3.0000e-01"),
            (0.30000000000000004_f64, 0, "3e-01"),
            (0.30000000000000004_f64, 1, "3.0e-01"),
            (0.30000000000000004_f64, 10, "3.0000000000e-01"),
            (3.0_f64, 4, "3.0000e+00"),
            (3.0_f64, 0, "3e+00"),
            (3.0_f64, 1, "3.0e+00"),
            (3.0_f64, 10, "3.0000000000e+00"),
            (123456789.0_f64, 4, "1.2346e+08"),
            (123456789.0_f64, 0, "1e+08"),
            (123456789.0_f64, 1, "1.2e+08"),
            (123456789.0_f64, 10, "1.2345678900e+08"),
            (1e-05_f64, 4, "1.0000e-05"),
            (1e-05_f64, 0, "1e-05"),
            (1e-05_f64, 1, "1.0e-05"),
            (1e-05_f64, 10, "1.0000000000e-05"),
        ];
        for &(x, p, e) in es {
            assert_eq!(format_e(x, p), e, "format({x:?}, '.{p}e')");
        }
        let fs: &[(f64, usize, &str)] = &[
            (0.0_f64, 0, "0"),
            (0.0_f64, 2, "0.00"),
            (0.0_f64, 6, "0.000000"),
            (-0.0_f64, 0, "-0"),
            (-0.0_f64, 2, "-0.00"),
            (-0.0_f64, 6, "-0.000000"),
            (0.5_f64, 0, "0"),
            (0.5_f64, 2, "0.50"),
            (0.5_f64, 6, "0.500000"),
            (1.5_f64, 0, "2"),
            (1.5_f64, 2, "1.50"),
            (1.5_f64, 6, "1.500000"),
            (2.5_f64, 0, "2"),
            (2.5_f64, 2, "2.50"),
            (2.5_f64, 6, "2.500000"),
            (0.125_f64, 0, "0"),
            (0.125_f64, 2, "0.12"),
            (0.125_f64, 6, "0.125000"),
            (0.375_f64, 0, "0"),
            (0.375_f64, 2, "0.38"),
            (0.375_f64, 6, "0.375000"),
            (2.675_f64, 0, "3"),
            (2.675_f64, 2, "2.67"),
            (2.675_f64, 6, "2.675000"),
            (1e+22_f64, 0, "10000000000000000000000"),
            (1e+22_f64, 2, "10000000000000000000000.00"),
            (1e+22_f64, 6, "10000000000000000000000.000000"),
            (1e-07_f64, 0, "0"),
            (1e-07_f64, 2, "0.00"),
            (1e-07_f64, 6, "0.000000"),
            (-0.001_f64, 0, "-0"),
            (-0.001_f64, 2, "-0.00"),
            (-0.001_f64, 6, "-0.001000"),
            (123.456_f64, 0, "123"),
            (123.456_f64, 2, "123.46"),
            (123.456_f64, 6, "123.456000"),
            (1000000000000000.0_f64, 0, "1000000000000000"),
            (1000000000000000.0_f64, 2, "1000000000000000.00"),
            (1000000000000000.0_f64, 6, "1000000000000000.000000"),
            (0.1_f64, 0, "0"),
            (0.1_f64, 2, "0.10"),
            (0.1_f64, 6, "0.100000"),
            (0.3333333333333333_f64, 0, "0"),
            (0.3333333333333333_f64, 2, "0.33"),
            (0.3333333333333333_f64, 6, "0.333333"),
            (99.995_f64, 0, "100"),
            (99.995_f64, 2, "100.00"),
            (99.995_f64, 6, "99.995000"),
            (f64::INFINITY, 0, "inf"),
            (f64::INFINITY, 2, "inf"),
            (f64::INFINITY, 6, "inf"),
            (f64::NEG_INFINITY, 0, "-inf"),
            (f64::NEG_INFINITY, 2, "-inf"),
            (f64::NEG_INFINITY, 6, "-inf"),
            (f64::NAN, 0, "nan"),
            (f64::NAN, 2, "nan"),
            (f64::NAN, 6, "nan"),
            (5e-324_f64, 0, "0"),
            (5e-324_f64, 2, "0.00"),
            (5e-324_f64, 6, "0.000000"),
            (0.05_f64, 0, "0"),
            (0.05_f64, 2, "0.05"),
            (0.05_f64, 6, "0.050000"),
            (0.15_f64, 0, "0"),
            (0.15_f64, 2, "0.15"),
            (0.15_f64, 6, "0.150000"),
            (0.25_f64, 0, "0"),
            (0.25_f64, 2, "0.25"),
            (0.25_f64, 6, "0.250000"),
            (0.35_f64, 0, "0"),
            (0.35_f64, 2, "0.35"),
            (0.35_f64, 6, "0.350000"),
            (1e-05_f64, 0, "0"),
            (1e-05_f64, 2, "0.00"),
            (1e-05_f64, 6, "0.000010"),
            (12345.6789_f64, 0, "12346"),
            (12345.6789_f64, 2, "12345.68"),
            (12345.6789_f64, 6, "12345.678900"),
            (-2.5_f64, 0, "-2"),
            (-2.5_f64, 2, "-2.50"),
            (-2.5_f64, 6, "-2.500000"),
            (1.005_f64, 0, "1"),
            (1.005_f64, 2, "1.00"),
            (1.005_f64, 6, "1.005000"),
            (1e+16_f64, 0, "10000000000000000"),
            (1e+16_f64, 2, "10000000000000000.00"),
            (1e+16_f64, 6, "10000000000000000.000000"),
            (3.0_f64, 0, "3"),
            (3.0_f64, 2, "3.00"),
            (3.0_f64, 6, "3.000000"),
        ];
        for &(x, p, e) in fs {
            assert_eq!(format_f(x, p), e, "format({x:?}, '.{p}f')");
        }
    }
}
