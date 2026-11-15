//! Python `re` semantics over `fancy_regex`.
//!
//! Patterns are written exactly as in Python and translated: `\s`/`\w` get Python's Latin-1
//! behaviour, `$` (without MULTILINE) also matches before a final `\n`, `\Z` is `\z`,
//! `(?P=name)` is a backreference, classes are made safe for Rust's nested-class syntax.
//! Offsets exposed by [`Match`] are CHAR offsets (Python `str` indices), never bytes. Empty
//! matches follow Python 3.7+ (`sub`, `split`, `finditer`).
//!
//! Not supported (not used by the port): VERBOSE, `\S`/`\W`/`\D` inside a class (panics),
//! `{,n}` quantifiers, `re.subn`, `Match.expand`, `endpos`.
//!
//! ```ignore
//! static IDENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z_]\w*", 0));
//! ```

use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

use fancy_regex::RegexInput;

/// `re.IGNORECASE`.
pub const IGNORECASE: u8 = 1;
/// `re.I`.
pub const I: u8 = IGNORECASE;
/// `re.MULTILINE`.
pub const MULTILINE: u8 = 2;
/// `re.M`.
pub const M: u8 = MULTILINE;
/// `re.DOTALL`.
pub const DOTALL: u8 = 4;
/// `re.S`.
pub const S: u8 = DOTALL;

/// Chars Python's `\w` matches in Latin-1 beyond what Rust's `\w` does (`str.isalnum`).
const EXTRA_W: &str = "\u{b2}\u{b3}\u{b9}\u{bc}\u{bd}\u{be}";
/// `\s` extras: U+001C..U+001F are whitespace to Python only.
const EXTRA_S: &str = "\\x1c-\\x1f";

/// Replace Python-only syntax and Unicode-class differences; see the module docs.
fn translate(pat: &str, multiline: bool) -> String {
    let chars: Vec<char> = pat.chars().collect();
    let mut out = String::with_capacity(pat.len() + 16);
    let mut i = 0;
    let (mut in_class, mut class_start) = (false, false);
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let n = *chars
                .get(i + 1)
                .unwrap_or_else(|| panic!("bad regex {pat:?}: trailing backslash"));
            i += 2;
            class_start = false;
            match n {
                's' if in_class => {
                    let _ = write!(out, "\\s{EXTRA_S}");
                }
                's' => {
                    let _ = write!(out, "[\\s{EXTRA_S}]");
                }
                'w' if in_class => {
                    let _ = write!(out, "\\w{EXTRA_W}");
                }
                'w' => {
                    let _ = write!(out, "[\\w{EXTRA_W}]");
                }
                'S' | 'W' | 'D' if in_class => {
                    panic!("regex {pat:?}: \\{n} inside a class is not supported")
                }
                'S' => {
                    let _ = write!(out, "[^\\s{EXTRA_S}]");
                }
                'W' => {
                    let _ = write!(out, "[^\\w{EXTRA_W}]");
                }
                'Z' => out.push_str("\\z"),
                'b' | 'B' if !in_class => {
                    let w = format!("[\\w{EXTRA_W}]");
                    // `\b`: word on exactly one side; `\B`: on both sides or neither.
                    let (c, d) = if n == 'b' {
                        ("(?!", "(?=")
                    } else {
                        ("(?=", "(?!")
                    };
                    {
                        let _ = write!(out, "(?:(?<={w}){c}{w})|(?<!{w}){d}{w}))");
                    }
                }
                n if n.is_whitespace() => {
                    let _ = write!(out, "\\x{{{:x}}}", u32::from(n));
                }
                n => {
                    out.push('\\');
                    out.push(n);
                }
            }
            continue;
        }
        i += 1;
        if in_class {
            match c {
                ']' if class_start => out.push_str("\\]"),
                ']' => {
                    in_class = false;
                    out.push(']');
                }
                '[' | '&' | '~' => {
                    out.push('\\');
                    out.push(c);
                }
                '^' if class_start => {
                    out.push('^');
                    continue;
                }
                c => out.push(c),
            }
            class_start = false;
            continue;
        }
        match c {
            '[' => {
                in_class = true;
                class_start = true;
                out.push('[');
            }
            '$' if !multiline => out.push_str("(?=\\n?\\z)"),
            '(' if chars[i..].starts_with(&['?', 'P', '=']) => {
                let end = chars[i..]
                    .iter()
                    .position(|&d| d == ')')
                    .unwrap_or_else(|| panic!("bad regex {pat:?}"));
                let name: String = chars[i + 3..i + end].iter().collect();
                {
                    let _ = write!(out, "\\k<{name}>");
                }
                i += end + 1;
            }
            c => out.push(c),
        }
    }
    out
}

/// True when the pattern starts with global inline flags containing `m` (`(?im)...`).
fn inline_multiline(pat: &str) -> bool {
    pat.strip_prefix("(?")
        .and_then(|r| r.split_once(')'))
        .is_some_and(|(flags, _)| {
            !flags.is_empty() && flags.chars().all(|c| "aiLmsux".contains(c)) && flags.contains('m')
        })
}

fn build(
    pat: &str,
    flags: u8,
    extra: impl FnOnce(&mut fancy_regex::RegexBuilder),
) -> fancy_regex::Result<fancy_regex::Regex> {
    let mut b = fancy_regex::RegexBuilder::new(pat);
    b.case_insensitive(flags & I != 0)
        .multi_line(flags & M != 0)
        .dot_matches_new_line(flags & S != 0);
    extra(&mut b);
    b.build()
}

/// A compiled pattern with Python `re` semantics. Build once, in a `LazyLock` static.
pub struct Regex {
    pattern: String,
    translated: String,
    flags: u8,
    re: fancy_regex::Regex,
    full: OnceLock<fancy_regex::Regex>,
    nonempty: OnceLock<Option<fancy_regex::Regex>>,
    names: Arc<[(String, usize)]>,
}

impl Regex {
    /// `re.compile(pattern, flags)`; panics, naming the pattern, when it does not compile.
    pub fn new(pattern: &str, flags: u8) -> Self {
        let translated = translate(pattern, flags & M != 0 || inline_multiline(pattern));
        let re = build(&translated, flags, |_| {})
            .unwrap_or_else(|e| panic!("regex {pattern:?} does not compile: {e}"));
        let names = re
            .capture_names()
            .enumerate()
            .filter_map(|(i, n)| n.map(|n| (n.to_owned(), i)))
            .collect();
        Self {
            pattern: pattern.to_owned(),
            translated,
            flags,
            re,
            full: OnceLock::new(),
            nonempty: OnceLock::new(),
            names,
        }
    }

    /// `pattern.pattern`.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    fn full(&self) -> &fancy_regex::Regex {
        self.full.get_or_init(|| {
            build(&format!("(?:{})\\z", self.translated), self.flags, |_| {})
                .unwrap_or_else(|e| panic!("regex {:?}: {e}", self.pattern))
        })
    }

    fn nonempty(&self) -> Option<&fancy_regex::Regex> {
        self.nonempty
            .get_or_init(|| {
                build(&self.translated, self.flags, |b| {
                    b.find_not_empty(true);
                })
                .ok()
            })
            .as_ref()
    }

    fn exec<'t>(
        &self,
        re: &fancy_regex::Regex,
        text: &'t str,
        pos: usize,
        anchored: bool,
    ) -> Option<Match<'t>> {
        let input = RegexInput::new(text).from_pos(pos).anchored(anchored);
        let caps = re
            .captures_input(input)
            .unwrap_or_else(|e| panic!("regex {:?} failed at run time: {e}", self.pattern))?;
        let spans = (0..caps.len())
            .map(|i| caps.get(i).map(|m| (m.start(), m.end())))
            .collect();
        Some(Match {
            text,
            spans,
            names: Arc::clone(&self.names),
            ascii: text.is_ascii(),
        })
    }

    /// `pattern.match(text)`: anchored at the start.
    pub fn match_<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.match_at(text, 0)
    }

    /// `pattern.match(text, pos)`; `pos` is a char index.
    pub fn match_at<'t>(&self, text: &'t str, pos: usize) -> Option<Match<'t>> {
        self.exec(&self.re, text, char_to_byte(text, pos), true)
    }

    /// `pattern.search(text)`.
    pub fn search<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.search_at(text, 0)
    }

    /// `pattern.search(text, pos)`; `pos` is a char index.
    pub fn search_at<'t>(&self, text: &'t str, pos: usize) -> Option<Match<'t>> {
        self.exec(&self.re, text, char_to_byte(text, pos), false)
    }

    /// `pattern.fullmatch(text)`.
    pub fn fullmatch<'t>(&self, text: &'t str) -> Option<Match<'t>> {
        self.exec(self.full(), text, 0, true)
    }

    /// Python 3.7+ iteration over successive matches; `f` returns false to stop.
    fn scan<'t>(&self, text: &'t str, mut f: impl FnMut(Match<'t>) -> bool) {
        let mut pos = 0;
        let mut prev_empty = false;
        while pos <= text.len() {
            let m = if prev_empty {
                // After an empty match the next one may not be empty at the same position.
                let same = self
                    .nonempty()
                    .and_then(|ne| self.exec(ne, text, pos, true));
                same.or_else(|| {
                    let next = pos + text[pos..].chars().next()?.len_utf8();
                    self.exec(&self.re, text, next, false)
                })
            } else {
                self.exec(&self.re, text, pos, false)
            };
            let Some(m) = m else { break };
            let (s, e) = m.spans[0].unwrap_or((pos, pos));
            prev_empty = s == e;
            pos = e;
            if !f(m) {
                break;
            }
        }
    }

    /// `list(pattern.finditer(text))`.
    pub fn finditer<'t>(&self, text: &'t str) -> Vec<Match<'t>> {
        let mut v = Vec::new();
        self.scan(text, |m| {
            v.push(m);
            true
        });
        v
    }

    /// `pattern.findall(text)` for patterns with at most one group (the whole match, or group 1;
    /// an unmatched group is `""`).
    pub fn findall<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let groups = self.re.captures_len() - 1;
        assert!(
            groups <= 1,
            "findall with several groups is not supported: {:?}",
            self.pattern
        );
        self.finditer(text)
            .iter()
            .map(|m| m.group(groups).unwrap_or(""))
            .collect()
    }

    /// `pattern.sub(repl, text)` with a Python replacement template (`\1`, `\g<name>`, `\n`...).
    pub fn sub(&self, repl: &str, text: &str) -> String {
        self.sub_n(repl, text, 0)
    }

    /// `pattern.sub(repl, text, count)`; `count` 0 replaces every match.
    pub fn sub_n(&self, repl: &str, text: &str, count: usize) -> String {
        let tpl = Template::parse(repl, &self.names, self.re.captures_len());
        self.sub_core(text, count, |m| tpl.expand(m))
    }

    /// `pattern.sub(f, text)` with a callable replacement.
    pub fn sub_fn(&self, text: &str, mut f: impl FnMut(&Match<'_>) -> String) -> String {
        self.sub_core(text, 0, |m| f(m))
    }

    fn sub_core(
        &self,
        text: &str,
        count: usize,
        mut f: impl FnMut(&Match<'_>) -> String,
    ) -> String {
        let mut out = String::with_capacity(text.len());
        let (mut last, mut n) = (0, 0);
        self.scan(text, |m| {
            let (s, e) = m.spans[0].unwrap_or((last, last));
            out.push_str(&text[last..s]);
            out.push_str(&f(&m));
            last = e;
            n += 1;
            count == 0 || n < count
        });
        out.push_str(&text[last..]);
        out
    }

    /// `pattern.split(text, maxsplit)` (`maxsplit` 0 = unlimited). Captured groups are included
    /// in the result; an unmatched group appears as `""` (Python gives `None`).
    pub fn split<'t>(&self, text: &'t str, maxsplit: usize) -> Vec<&'t str> {
        let mut out = Vec::new();
        let (mut last, mut n) = (0, 0);
        self.scan(text, |m| {
            let (s, e) = m.spans[0].unwrap_or((last, last));
            out.push(&text[last..s]);
            for g in 1..m.spans.len() {
                out.push(m.group(g).unwrap_or(""));
            }
            last = e;
            n += 1;
            maxsplit == 0 || n < maxsplit
        });
        out.push(&text[last..]);
        out
    }
}

/// `re.escape(s)` (Python 3.7+: only regex-special chars are escaped).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn char_to_byte(text: &str, pos: usize) -> usize {
    if text.is_ascii() {
        pos.min(text.len())
    } else {
        text.char_indices().nth(pos).map_or(text.len(), |(b, _)| b)
    }
}

/// A successful match. All offsets are char offsets into the searched text.
pub struct Match<'t> {
    text: &'t str,
    spans: Vec<Option<(usize, usize)>>,
    names: Arc<[(String, usize)]>,
    ascii: bool,
}

impl<'t> Match<'t> {
    fn cidx(&self, byte: usize) -> usize {
        if self.ascii {
            byte
        } else {
            self.text[..byte].chars().count()
        }
    }

    /// `m.group(i)`; `None` for an unmatched group (and for an index past the last group, where
    /// Python raises `IndexError`).
    pub fn group(&self, i: usize) -> Option<&'t str> {
        self.spans
            .get(i)
            .copied()
            .flatten()
            .map(|(s, e)| &self.text[s..e])
    }

    /// `m.group()`: the whole match.
    pub fn group0(&self) -> &'t str {
        self.group(0).unwrap_or("")
    }

    /// `m.group(name)`; panics on an unknown name (Python's `IndexError`).
    pub fn named(&self, name: &str) -> Option<&'t str> {
        self.group(self.name_index(name))
    }

    fn name_index(&self, name: &str) -> usize {
        self.names
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no group named {name:?}"))
            .1
    }

    /// `m.start(i)` (`None` where Python gives -1).
    pub fn start(&self, i: usize) -> Option<usize> {
        self.span(i).map(|s| s.0)
    }

    /// `m.end(i)`.
    pub fn end(&self, i: usize) -> Option<usize> {
        self.span(i).map(|s| s.1)
    }

    /// `m.span(i)`.
    pub fn span(&self, i: usize) -> Option<(usize, usize)> {
        self.spans
            .get(i)
            .copied()
            .flatten()
            .map(|(s, e)| (self.cidx(s), self.cidx(e)))
    }

    /// `m.start()`.
    pub fn start0(&self) -> usize {
        self.start(0).unwrap_or(0)
    }

    /// `m.end()`.
    pub fn end0(&self) -> usize {
        self.end(0).unwrap_or(0)
    }

    /// `m.span()`.
    pub fn span0(&self) -> (usize, usize) {
        (self.start0(), self.end0())
    }

    /// `m.lastgroup` for patterns that are an alternation of named groups: the matched named
    /// group that ends last.
    pub fn lastgroup(&self) -> Option<&str> {
        self.names
            .iter()
            .filter_map(|(n, i)| self.spans[*i].map(|(_, e)| (e, *i, n.as_str())))
            .max_by_key(|&(e, i, _)| (e, std::cmp::Reverse(i)))
            .map(|t| t.2)
    }
}

enum Piece {
    Lit(String),
    Group(usize),
}

/// A parsed Python replacement template.
struct Template(Vec<Piece>);

impl Template {
    fn parse(repl: &str, names: &[(String, usize)], ngroups: usize) -> Self {
        let chars: Vec<char> = repl.chars().collect();
        let mut pieces = Vec::new();
        let mut lit = String::new();
        let mut i = 0;
        let check = |g: usize| {
            assert!(g < ngroups, "invalid group reference {g} in {repl:?}");
            g
        };
        while i < chars.len() {
            if chars[i] != '\\' {
                lit.push(chars[i]);
                i += 1;
                continue;
            }
            let n = *chars
                .get(i + 1)
                .unwrap_or_else(|| panic!("bad escape (end of pattern) in {repl:?}"));
            i += 2;
            match n {
                'g' => {
                    assert!(chars.get(i) == Some(&'<'), "missing < in {repl:?}");
                    let end = chars[i..]
                        .iter()
                        .position(|&c| c == '>')
                        .unwrap_or_else(|| panic!("missing > in {repl:?}"));
                    let name: String = chars[i + 1..i + end].iter().collect();
                    i += end + 1;
                    let g = name
                        .parse::<usize>()
                        .ok()
                        .or_else(|| names.iter().find(|(k, _)| *k == name).map(|(_, g)| *g));
                    let g = g.unwrap_or_else(|| panic!("unknown group name {name:?} in {repl:?}"));
                    pieces.push(Piece::Lit(std::mem::take(&mut lit)));
                    pieces.push(Piece::Group(check(g)));
                }
                '0' => lit.push('\0'),
                '1'..='9' => {
                    let mut g = n.to_digit(10).unwrap_or(0) as usize;
                    if let Some(d) = chars.get(i).and_then(|c| c.to_digit(10)) {
                        g = g * 10 + d as usize;
                        i += 1;
                    }
                    pieces.push(Piece::Lit(std::mem::take(&mut lit)));
                    pieces.push(Piece::Group(check(g)));
                }
                'n' => lit.push('\n'),
                't' => lit.push('\t'),
                'r' => lit.push('\r'),
                'f' => lit.push('\x0c'),
                'v' => lit.push('\x0b'),
                'a' => lit.push('\x07'),
                'b' => lit.push('\x08'),
                '\\' => lit.push('\\'),
                c if c.is_ascii_alphabetic() => panic!("bad escape \\{c} in {repl:?}"),
                c => {
                    lit.push('\\');
                    lit.push(c);
                }
            }
        }
        pieces.push(Piece::Lit(lit));
        Self(pieces)
    }

    fn expand(&self, m: &Match<'_>) -> String {
        self.0
            .iter()
            .map(|p| match p {
                Piece::Lit(s) => s.as_str(),
                Piece::Group(g) => m.group(*g).unwrap_or(""),
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(
    clippy::unreadable_literal,
    reason = "expected values are CPython output, pasted verbatim"
)]
mod tests {
    use super::*;

    fn re(p: &str, f: u8) -> Regex {
        Regex::new(p, f)
    }

    type Shown<'a> = (usize, usize, Vec<Option<&'a str>>);

    fn sh(m: Option<Match<'_>>) -> Option<Shown<'_>> {
        m.map(|m| {
            (
                m.start0(),
                m.end0(),
                (1..m.spans.len()).map(|i| m.group(i)).collect(),
            )
        })
    }

    fn shv(v: Vec<Match<'_>>) -> Vec<Shown<'_>> {
        v.into_iter().filter_map(|m| sh(Some(m))).collect()
    }

    /// Expected values below were produced by running Python's `re` on the same inputs.
    #[test]
    fn matches_python_re() {
        assert_eq!(
            sh(re("\\s*(\\w+)\\s*(\\(?)", 0).match_("  foo (x")),
            Some((0, 7, vec![Some("foo"), Some("(")]))
        );
        assert_eq!(
            sh(re("\\s*(\\w+)\\s*(\\(?)", 0).match_("foo")),
            Some((0, 3, vec![Some("foo"), Some("")]))
        );
        assert_eq!(sh(re("\\s*(\\w+)\\s*(\\(?)", 0).match_("")), None);
        assert_eq!(
            sh(re("\\s*(\\w+)\\s*(\\(?)", 0).match_("\u{e9}t\u{e9} x")),
            Some((0, 4, vec![Some("\u{e9}t\u{e9}"), Some("")]))
        );
        assert_eq!(
            sh(re("\\s*(\\w+)\\s*(\\(?)", 0).match_("x")),
            Some((0, 1, vec![Some("x"), Some("")]))
        );
        assert_eq!(
            sh(re("[A-Za-z_]\\w*\\$?", 0).match_("abc$ rest")),
            Some((0, 4, vec![]))
        );
        assert_eq!(sh(re("[A-Za-z_]\\w*\\$?", 0).match_(" abc")), None);
        assert_eq!(
            sh(re("[A-Za-z_]\\w*\\$?", 0).search("x\u{e9}y")),
            Some((0, 3, vec![]))
        );
        assert_eq!(sh(re("\\w+", 0).match_at("ab cd", 3)), Some((3, 5, vec![])));
        assert_eq!(sh(re("\\w+", 0).match_at("ab cd", 2)), None);
        assert_eq!(sh(re("\\w+", 0).match_at("ab cd", 99)), None);
        assert_eq!(sh(re("x*", 0).match_at("abc", 3)), Some((3, 3, vec![])));
        assert_eq!(sh(re("x*", 0).match_at("abc", 9)), Some((3, 3, vec![])));
        assert_eq!(
            sh(re("\\w+", 0).match_at("\u{e9}\u{e9} ab\u{b0}cd", 3)),
            Some((3, 5, vec![]))
        );
        assert_eq!(
            sh(re("\\w+", 0).search_at("\u{e9}\u{e9} ab\u{b0}cd", 3)),
            Some((3, 5, vec![]))
        );
        assert_eq!(sh(re("^b", 0).search_at("ab", 1)), None);
        assert_eq!(sh(re("^b", 0).match_at("ab", 1)), None);
        assert_eq!(sh(re("\\bb", 0).search_at("ab", 1)), None);
        assert_eq!(sh(re("(?<=a)b", 0).match_at("ab", 1)), Some((1, 2, vec![])));
        assert_eq!(
            sh(re("\\w+", 0).search_at("ab cd", 2)),
            Some((3, 5, vec![]))
        );
        assert_eq!(sh(re("a$", 0).search("a\n")), Some((0, 1, vec![])));
        assert_eq!(sh(re("a$", 0).search("a\n\n")), None);
        assert_eq!(sh(re("a$", 0).search("a")), Some((0, 1, vec![])));
        assert_eq!(sh(re("a$", M).search("a\nb")), Some((0, 1, vec![])));
        assert_eq!(sh(re("a$", 0).search("a\nb")), None);
        assert_eq!(sh(re("^b", M).search("a\nb")), Some((2, 3, vec![])));
        assert_eq!(sh(re("^b", 0).search("a\nb")), None);
        assert_eq!(sh(re("(?m)a$", 0).search("a\nb")), Some((0, 1, vec![])));
        assert_eq!(sh(re("a\\Z", 0).search("a\n")), None);
        assert_eq!(sh(re("a\\Z", 0).search("a")), Some((0, 1, vec![])));
        assert_eq!(sh(re("a$", 0).fullmatch("a")), Some((0, 1, vec![])));
        assert_eq!(sh(re("a$", 0).fullmatch("a\n")), None);
        assert_eq!(
            sh(re("^\\s*(?:Public|Dim)\\b", I).match_("  dim x")),
            Some((0, 5, vec![]))
        );
        assert_eq!(
            sh(re("[0-9][0-9A-Za-z_]*(?:[-+*/^][0-9A-Za-z_]+)+\\Z", 0).search("1+2")),
            Some((0, 3, vec![]))
        );
        assert_eq!(
            sh(re("[0-9][0-9A-Za-z_]*(?:[-+*/^][0-9A-Za-z_]+)+\\Z", 0).search("1+2\n")),
            None
        );
        assert_eq!(sh(re("[+-]?\\d+$", 0).match_("12\n")), Some((0, 2, vec![])));
        assert_eq!(
            sh(re("[+-]?\\d+$", 0).fullmatch("12")),
            Some((0, 2, vec![]))
        );
        assert_eq!(sh(re("a|ab", 0).fullmatch("ab")), Some((0, 2, vec![])));
        assert_eq!(sh(re("a*?", 0).fullmatch("aaa")), Some((0, 3, vec![])));
        assert_eq!(
            sh(re("(a+)(b)?", 0).fullmatch("aab")),
            Some((0, 3, vec![Some("aa"), Some("b")]))
        );
        assert_eq!(sh(re("(a+)(b)?", 0).fullmatch("aabc")), None);
        assert_eq!(sh(re("", 0).fullmatch("")), Some((0, 0, vec![])));
        assert_eq!(sh(re("", 0).fullmatch("a")), None);
        assert_eq!(
            sh(re("\\w+", 0).match_("a\u{b2}\u{b3}b\u{b9}\u{bc}\u{bd}\u{be} c")),
            Some((0, 8, vec![]))
        );
        assert_eq!(
            sh(re("\\w+", 0).match_("\u{aa}\u{b5}\u{ba}\u{c0}\u{ff}\u{d7}\u{f7}x")),
            Some((0, 5, vec![]))
        );
        assert_eq!(sh(re("\\d+", 0).match_("12\u{b2}3")), Some((0, 2, vec![])));
        assert_eq!(
            sh(re("\\s+", 0).match_("\u{1c}\u{1d}\u{1e}\u{1f} \u{a0}\u{85}\t\nx")),
            Some((0, 9, vec![]))
        );
        assert_eq!(sh(re("\\S+", 0).match_("ab\u{1c}c")), Some((0, 2, vec![])));
        assert_eq!(
            sh(re("\\W+", 0).match_("\u{d7} \u{b0}\u{e9}")),
            Some((0, 3, vec![]))
        );
        assert_eq!(
            sh(re("[\\s,]+", 0).match_(",\u{1f} ,x")),
            Some((0, 4, vec![]))
        );
        assert_eq!(
            sh(re("[\\w.]+", 0).match_("a\u{b2}.\u{b0}")),
            Some((0, 3, vec![]))
        );
        assert_eq!(
            sh(re("[^\\s,]+", 0).match_("ab\u{1f}c")),
            Some((0, 2, vec![]))
        );
        assert_eq!(
            sh(re("[\\d.\\s]+", 0).match_("1.\u{1c}2x")),
            Some((0, 4, vec![]))
        );
        assert_eq!(sh(re("\\bx", 0).search("\u{b2}x")), None);
        assert_eq!(sh(re("\\bx", 0).search("\u{e9}x")), None);
        assert_eq!(sh(re("\\bx", 0).search("\u{b0}x")), Some((1, 2, vec![])));
        assert_eq!(sh(re("x\\b", 0).search("x\u{b2}")), None);
        assert_eq!(sh(re("x\\b", 0).search("x\u{b0}")), Some((0, 1, vec![])));
        assert_eq!(sh(re("\\Bx", 0).search("ax")), Some((1, 2, vec![])));
        assert_eq!(sh(re("\\Bx", 0).search(" x")), None);
        assert_eq!(sh(re("\\b\\w+\\b", 0).search("")), None);
        assert_eq!(sh(re("\\xe9", I).match_("\u{c9}")), Some((0, 1, vec![])));
        assert_eq!(
            sh(re("(?i)then", 0).search("x\u{c9}THEN")),
            Some((2, 6, vec![]))
        );
        assert_eq!(sh(re("then", I).fullmatch("THEN")), Some((0, 4, vec![])));
        assert_eq!(
            sh(re("(?i:and|or)x", 0).match_("ORx")),
            Some((0, 3, vec![]))
        );
        assert_eq!(sh(re("(?i:and|or)x", 0).match_("ORX")), None);
        assert_eq!(
            sh(re("(?i)\\bThen\\s*$", 0).search("if x Then  ")),
            Some((5, 11, vec![]))
        );
        assert_eq!(
            sh(re("(?i)\\bThen\\s*$", 0).search("if x Then\n")),
            Some((5, 10, vec![]))
        );
        assert_eq!(sh(re("a.b", 0).match_("a\nb")), None);
        assert_eq!(sh(re("a.b", S).match_("a\nb")), Some((0, 3, vec![])));
        assert_eq!(sh(re("(?s)a.b", 0).match_("a\nb")), Some((0, 3, vec![])));
        assert_eq!(
            sh(re("(?is)^\\s*If\\b.*?\\bThen\\b(\\s*\\S.*)$", 0).match_("if a\nThen b\n")),
            Some((0, 12, vec![Some(" b\n")]))
        );
        assert_eq!(
            sh(re("(?is)^\\s*If(?:\\s|(?=\\())(.*)$", 0).match_("IF(x) Then")),
            Some((0, 10, vec![Some("(x) Then")]))
        );
        assert_eq!(
            sh(re(
                "(?i)^\\s*For\\s(.*?)=(.*?)\\sTo\\s(.*?)(?:\\sStep\\s(.*))?$",
                0
            )
            .match_("For i = 1 To 10 Step 2")),
            Some((0, 22, vec![Some("i "), Some(" 1"), Some("10"), Some("2")]))
        );
        assert_eq!(
            sh(re(
                "(?i)^\\s*For\\s(.*?)=(.*?)\\sTo\\s(.*?)(?:\\sStep\\s(.*))?$",
                0
            )
            .match_("For i = 1 To 10")),
            Some((0, 15, vec![Some("i "), Some(" 1"), Some("10"), None]))
        );
        assert_eq!(
            sh(re(
                "\\s*(?:\\((?:[^()]|\\((?:[^()]|\\([^()]*\\))*\\))*\\)\\s*)?[-+*/\\\\^&]?=",
                0
            )
            .search("(1,(2),((3))) = 5")),
            Some((0, 15, vec![]))
        );
        assert_eq!(
            sh(re(
                "[+-]?(\\d+\\.\\d*|\\.\\d+)([eE][+-]?\\d+)?$|[+-]?\\d+[eE][+-]?\\d+$",
                0
            )
            .match_("1.5e3")),
            Some((0, 5, vec![Some("1.5"), Some("e3")]))
        );
        assert_eq!(
            sh(re(
                "[+-]?(\\d+\\.\\d*|\\.\\d+)([eE][+-]?\\d+)?$|[+-]?\\d+[eE][+-]?\\d+$",
                0
            )
            .match_("-3E4")),
            Some((0, 4, vec![None, None]))
        );
        assert_eq!(
            sh(re(
                "[+-]?(\\d+\\.\\d*|\\.\\d+)([eE][+-]?\\d+)?$|[+-]?\\d+[eE][+-]?\\d+$",
                0
            )
            .match_("12")),
            None
        );
        assert_eq!(
            sh(re("(\\d+\\.\\d*|\\.\\d+|\\d+)([Ee][+-]?\\d+)?", 0).match_("12.5e+3x")),
            Some((0, 7, vec![Some("12.5"), Some("e+3")]))
        );
        assert_eq!(
            sh(re("\\\"[^\\\"]*\\\"|'.*$", 0).search("a \"s\" 'c")),
            Some((2, 5, vec![]))
        );
        assert_eq!(
            sh(re("^\\s*Include\\s*\\(?\\s*\"([^\"]*)\"\\s*\\)?\\s*$", I)
                .match_("  include(\"a b.dld\")")),
            Some((0, 20, vec![Some("a b.dld")]))
        );
        assert_eq!(
            sh(re("^(CPU|USR|CRD|USB):", I).match_("usr:file")),
            Some((0, 4, vec![Some("usr")]))
        );
        assert_eq!(
            sh(re(
                "\\s*Const\\s+([A-Za-z_]\\w*)\\(\\s*([^\\s,()]+(?:\\s+[^\\s,()]+)*)\\s*[,)]",
                I
            )
            .match_("const a( x y , 2)")),
            Some((0, 14, vec![Some("a"), Some("x y")]))
        );
        assert_eq!(
            sh(
                re("^\\s*(?:Include(?:Section)?(?=[\\s(=])|Include\\s*$)", I)
                    .match_("IncludeSection(")
            ),
            Some((0, 14, vec![]))
        );
        assert_eq!(
            sh(re("[\\w\\\")]\\s+[\\w\\\"]", 0).search("f(x) y")),
            Some((3, 6, vec![]))
        );
        assert_eq!(sh(re("(?:[ \\t]*(?:<>|<=|>=|[-+*/^&<>=])[ \\t]*)(?=[A-Za-z_0-9\\\"])|(?:<>|<=|>=|[-+*/^&<>=])", 0).match_("  <= x")), Some((0, 5, vec![])));
        assert_eq!(sh(re("[]a]+", 0).match_("a]a[")), Some((0, 3, vec![])));
        assert_eq!(sh(re("[[a]+", 0).match_("a[a]")), Some((0, 3, vec![])));
        assert_eq!(
            sh(re("[A-Za-z0-9_.&+\\-*/ ]+", 0).fullmatch("a.b&c-d* /e!")),
            None
        );
        assert_eq!(sh(re("[a&&b]+", 0).match_("a&b&&")), Some((0, 5, vec![])));
        assert_eq!(sh(re("[a~~b]+", 0).match_("a~b")), Some((0, 3, vec![])));
        assert_eq!(sh(re("[^]a]+", 0).match_("xy]")), Some((0, 2, vec![])));
        assert_eq!(sh(re("\\[a\\]", 0).match_("[a]")), Some((0, 3, vec![])));
        assert_eq!(
            sh(re("\\\"\\-\\&\\#\\~", 0).match_("\"-&#~")),
            Some((0, 5, vec![]))
        );
        assert_eq!(
            sh(re("(?P<q>['\\\"]).*?(?P=q)", 0).search("x 'a\"b' y")),
            Some((2, 7, vec![Some("'")]))
        );
        assert_eq!(
            sh(re("(?P<a>x)(?P<b>y)?", 0).match_("xz")),
            Some((0, 1, vec![Some("x"), None]))
        );
        assert_eq!(
            sh(re("(a)|(b)", 0).match_("b")),
            Some((0, 1, vec![None, Some("b")]))
        );
        assert_eq!(sh(re("(a)?b", 0).match_("b")), Some((0, 1, vec![None])));
        assert_eq!(
            sh(re("\\xe9+", 0).search("a\u{e9}\u{e9}\u{b0}b")),
            Some((1, 3, vec![]))
        );
        assert_eq!(
            sh(re("(\\xb0)(b)", 0).search("a\u{e9}\u{e9}\u{b0}b")),
            Some((3, 5, vec![Some("\u{b0}"), Some("b")]))
        );
        assert_eq!(
            sh(re("b$", 0).search("\u{e9}\u{e9}b\n")),
            Some((2, 3, vec![]))
        );
        assert_eq!(
            sh(re("x", 0).search("\u{394}\u{3a9}x")),
            Some((2, 3, vec![]))
        );
        assert_eq!(
            shv(re("\\b\\w+\\b", 0).finditer("n\u{e9} ab\u{b2}c d")),
            vec![(0, 2, vec![]), (3, 7, vec![]), (8, 9, vec![])]
        );
        assert_eq!(
            shv(re("^|\\w+", 0).finditer("foo bar")),
            vec![(0, 0, vec![]), (0, 3, vec![]), (4, 7, vec![])]
        );
        assert_eq!(
            shv(re("^|\\w+", M).finditer("foo\nbar")),
            vec![
                (0, 0, vec![]),
                (0, 3, vec![]),
                (4, 4, vec![]),
                (4, 7, vec![])
            ]
        );
        assert_eq!(
            shv(re("x*", 0).finditer("axbxx")),
            vec![
                (0, 0, vec![]),
                (1, 2, vec![]),
                (2, 2, vec![]),
                (3, 5, vec![]),
                (5, 5, vec![])
            ]
        );
        assert_eq!(
            shv(re("", 0).finditer("ab")),
            vec![(0, 0, vec![]), (1, 1, vec![]), (2, 2, vec![])]
        );
        assert_eq!(shv(re("", 0).finditer("")), vec![(0, 0, vec![])]);
        assert_eq!(
            shv(re("|a", 0).finditer("a")),
            vec![(0, 0, vec![]), (0, 1, vec![]), (1, 1, vec![])]
        );
        assert_eq!(
            shv(re("a|", 0).finditer("ba")),
            vec![(0, 0, vec![]), (1, 2, vec![]), (2, 2, vec![])]
        );
        assert_eq!(
            shv(re("\\s*", 0).finditer("a  b")),
            vec![
                (0, 0, vec![]),
                (1, 3, vec![]),
                (3, 3, vec![]),
                (4, 4, vec![])
            ]
        );
        assert_eq!(
            shv(re("$", 0).finditer("a\n")),
            vec![(1, 1, vec![]), (2, 2, vec![])]
        );
        assert_eq!(
            shv(re("$", M).finditer("a\nb\n")),
            vec![(1, 1, vec![]), (3, 3, vec![]), (4, 4, vec![])]
        );
        assert_eq!(
            shv(re("(a)|b", 0).finditer("ab")),
            vec![(0, 1, vec![Some("a")]), (1, 2, vec![None])]
        );
        assert_eq!(
            shv(re("(\\w)\\s*\\(", 0).finditer("f (x) g(y)")),
            vec![(0, 3, vec![Some("f")]), (6, 8, vec![Some("g")])]
        );
        assert_eq!(
            shv(re("\\d+", 0).finditer("\u{e9} 12 \u{b0} 345")),
            vec![(2, 4, vec![]), (7, 10, vec![])]
        );
        assert_eq!(
            shv(re("(?i)\\b(IfTime|TimeIntoInterval|Timer)\\b\\s*\\(", 0)
                .finditer("x = iftime (1) + Timer(2)")),
            vec![(4, 12, vec![Some("iftime")]), (17, 23, vec![Some("Timer")])]
        );
        assert_eq!(
            shv(re("\\b", 0).finditer("ab cd")),
            vec![
                (0, 0, vec![]),
                (2, 2, vec![]),
                (3, 3, vec![]),
                (5, 5, vec![])
            ]
        );
        assert_eq!(shv(re("\\B", 0).finditer("ab")), vec![(1, 1, vec![])]);
        assert_eq!(
            re("[A-Za-z_]\\w*", 0).findall("x1 = y_2 + 3z"),
            vec!["x1", "y_2", "z"]
        );
        assert_eq!(
            re("([A-Za-z_]\\w*)\\s*\\(", 0).findall("f(x) + g (y) + z"),
            vec!["f", "g"]
        );
        assert_eq!(re("\\d+|x", 0).findall("a1x22"), vec!["1", "x", "22"]);
        assert_eq!(re("|a", 0).findall("a"), vec!["", "a", ""]);
        assert_eq!(re("(a)|b", 0).findall("ab"), vec!["a", ""]);
        assert_eq!(re("\\w+", 0).findall(""), Vec::<&str>::new());
        assert_eq!(
            re("\\w+", 0).findall("\u{e9}\u{b2} \u{b0} ab"),
            vec!["\u{e9}\u{b2}", "ab"]
        );
        assert_eq!(
            re("(?i)\\bAs\\s+([A-Za-z_][A-Za-z0-9_]*)", 0).findall("a As Long, b as String"),
            vec!["Long", "String"]
        );
        assert_eq!(re("(a)(b)?", 0).sub("[\\1|\\2]", "ab a"), "[a|b] [a|]");
        assert_eq!(re("(a)(b)?", 0).sub_n("[\\1|\\2]", "ab a", 1), "[a|b] a");
        assert_eq!(re("(?P<x>a)", 0).sub("<\\g<x>\\g<0>>", "aa"), "<aa><aa>");
        assert_eq!(re("(a)", 0).sub("\\g<1>1", "a"), "a1");
        assert_eq!(re("a", 0).sub("\\\\", "a"), "\\");
        assert_eq!(re("a", 0).sub("\\n|\\t", "a"), "\n|\t");
        assert_eq!(re("a", 0).sub("\\.", "a"), "\\.");
        assert_eq!(re("a", 0).sub("", "banana"), "bnn");
        assert_eq!(re("a", 0).sub_n("", "banana", 2), "bnna");
        assert_eq!(re("x*", 0).sub("-", "abxd"), "-a-b--d-");
        assert_eq!(re("", 0).sub("-", "ab"), "-a-b-");
        assert_eq!(re("", 0).sub("-", ""), "-");
        assert_eq!(re("|a", 0).sub("-", "a"), "---");
        assert_eq!(re("\\s*", 0).sub("_", "a b"), "_a__b_");
        assert_eq!(re("$", 0).sub("!", "a\n"), "a!\n!");
        assert_eq!(re("$", M).sub("!", "a\nb"), "a!\nb!");
        assert_eq!(re("\\s+", 0).sub("", "a \u{a0}\u{1c} b"), "ab");
        assert_eq!(
            re("^\\s*call(?![A-Za-z0-9_])", I).sub("", "  CALL foo"),
            " foo"
        );
        assert_eq!(
            re("^(CPU|USR|CRD|USB):", I).sub("", "cpu:abc\u{e9}"),
            "abc\u{e9}"
        );
        assert_eq!(
            re("(?i)^\\s*Const\\b", 0).sub_n("", "  const X = 1", 1),
            " X = 1"
        );
        assert_eq!(
            re("^(Cannot open include file  )(?:CPU|USR|CRD|USB):", I)
                .sub("\\1", "Cannot open include file  USR:x.dld"),
            "Cannot open include file  x.dld"
        );
        assert_eq!(
            re(
                "^(?:Illegal parameter |[^:]*: )|(?:, expecting constant integer)?\\.$",
                0
            )
            .sub("", "Illegal parameter foo, expecting constant integer."),
            "foo"
        );
        assert_eq!(
            re(
                "(?i)^[ \\t]*(?:(?:and|or|xor|imp|eqv|to|not|mod|is)(?![A-Za-z0-9_])[ \\t]*)*",
                0
            )
            .sub_n("", "  And  or x", 1),
            "x"
        );
        assert_eq!(
            re("\"[^\"]*\"", 0).sub("", "a \"b\" c \"d\u{e9}\""),
            "a  c "
        );
        assert_eq!(re("\\xe9", 0).sub("e", "caf\u{e9} \u{e9}\u{e9}"), "cafe ee");
        assert_eq!(
            re("\"[^\"]*\"", 0).sub_fn("a \"b\u{e9}c\" d \"\" e", |m| "s"
                .repeat(m.group0().chars().count())),
            "a sssss d ss e"
        );
        assert_eq!(
            re("\"[^\"]*\"", 0).sub_fn("a \"bc\" d", |m| " ".repeat(m.group0().chars().count())),
            "a      d"
        );
        assert_eq!(
            re("(\\w)(\\w*)", 0).sub_fn("foo bar \u{e9}t\u{e9}", |m| m
                .group(1)
                .unwrap()
                .to_uppercase()
                + m.group(2).unwrap()),
            "Foo Bar \u{c9}t\u{e9}"
        );
        assert_eq!(
            re("x*", 0).sub_fn("a\u{e9}xb", |m| format!("<{},{}>", m.start0(), m.end0())),
            "<0,0>a<1,1>\u{e9}<2,3><3,3>b<4,4>"
        );
        assert_eq!(
            re("x*", 0).split("axbc", 0),
            vec!["", "a", "", "b", "c", ""]
        );
        assert_eq!(re("\\W", 0).split("ab.cd e", 1), vec!["ab", "cd e"]);
        assert_eq!(re("\\W", 0).split("ab.cd e", 0), vec!["ab", "cd", "e"]);
        assert_eq!(re("\\W", 0).split("  abc", 1), vec!["", " abc"]);
        assert_eq!(re("\\W", 0).split("", 1), vec![""]);
        assert_eq!(re(",", 0).split(",a,,b,", 0), vec!["", "a", "", "b", ""]);
        assert_eq!(
            re("(,)", 0).split("a,b,c", 0),
            vec!["a", ",", "b", ",", "c"]
        );
        assert_eq!(re("(,)", 0).split("a,b,c", 1), vec!["a", ",", "b,c"]);
        assert_eq!(re("(,)|(;)", 0).split("a,b", 0), vec!["a", ",", "", "b"]);
        assert_eq!(
            re("\\sStep\\s", I).split("1 To 5 STEP 2 step 3", 1),
            vec!["1 To 5", "2 step 3"]
        );
        assert_eq!(re("\\sStep\\s", I).split("1 To 5", 0), vec!["1 To 5"]);
        assert_eq!(
            re("\\b", 0).split("ab cd", 0),
            vec!["", "ab", " ", "cd", ""]
        );
        assert_eq!(re("", 0).split("ab", 0), vec!["", "a", "b", ""]);
        assert_eq!(re("", 0).split("", 0), vec!["", ""]);
        assert_eq!(
            re("\\s+", 0).split("a \u{a0} b\u{1c}c", 0),
            vec!["a", "b", "c"]
        );
        assert_eq!(
            re("\\xe9", 0).split("a\u{e9}b\u{e9}c", 1),
            vec!["a", "b\u{e9}c"]
        );
        assert_eq!(re("^", M).split("a\nb\nc", 0), vec!["", "a\n", "b\n", "c"]);
        assert_eq!(re("$", M).split("a\nb", 0), vec!["a", "\nb", ""]);
        assert_eq!(escape("abc"), "abc");
        assert_eq!(
            re(&escape("abc"), 0).fullmatch("abc").map(|m| m.span0()),
            Some((0, 3))
        );
        assert_eq!(escape("a.b"), "a\\.b");
        assert_eq!(
            re(&escape("a.b"), 0).fullmatch("a.b").map(|m| m.span0()),
            Some((0, 3))
        );
        assert_eq!(escape("a b#"), "a\\ b\\#");
        assert_eq!(
            re(&escape("a b#"), 0).fullmatch("a b#").map(|m| m.span0()),
            Some((0, 4))
        );
        assert_eq!(escape("x-y"), "x\\-y");
        assert_eq!(
            re(&escape("x-y"), 0).fullmatch("x-y").map(|m| m.span0()),
            Some((0, 3))
        );
        assert_eq!(escape("&~"), "\\&\\~");
        assert_eq!(
            re(&escape("&~"), 0).fullmatch("&~").map(|m| m.span0()),
            Some((0, 2))
        );
        assert_eq!(escape("\t\n"), "\\\t\\\n");
        assert_eq!(
            re(&escape("\t\n"), 0).fullmatch("\t\n").map(|m| m.span0()),
            Some((0, 2))
        );
        assert_eq!(escape("\u{e9}\u{b0}"), "\u{e9}\u{b0}");
        assert_eq!(
            re(&escape("\u{e9}\u{b0}"), 0)
                .fullmatch("\u{e9}\u{b0}")
                .map(|m| m.span0()),
            Some((0, 2))
        );
        assert_eq!(escape("_"), "_");
        assert_eq!(
            re(&escape("_"), 0).fullmatch("_").map(|m| m.span0()),
            Some((0, 1))
        );
        assert_eq!(escape("a+b(c)[d]{e}"), "a\\+b\\(c\\)\\[d\\]\\{e\\}");
        assert_eq!(
            re(&escape("a+b(c)[d]{e}"), 0)
                .fullmatch("a+b(c)[d]{e}")
                .map(|m| m.span0()),
            Some((0, 12))
        );
        assert_eq!(escape("\\"), "\\\\");
        assert_eq!(
            re(&escape("\\"), 0).fullmatch("\\").map(|m| m.span0()),
            Some((0, 1))
        );
        assert_eq!(escape("a|b^c$"), "a\\|b\\^c\\$");
        assert_eq!(
            re(&escape("a|b^c$"), 0)
                .fullmatch("a|b^c$")
                .map(|m| m.span0()),
            Some((0, 6))
        );
        assert_eq!(escape("A=B?*"), "A=B\\?\\*");
        assert_eq!(
            re(&escape("A=B?*"), 0)
                .fullmatch("A=B?*")
                .map(|m| m.span0()),
            Some((0, 5))
        );
        assert_eq!(escape("\"q\""), "\"q\"");
        assert_eq!(
            re(&escape("\"q\""), 0)
                .fullmatch("\"q\"")
                .map(|m| m.span0()),
            Some((0, 3))
        );
        assert_eq!(escape("<>:;,!%@/'`"), "<>:;,!%@/'`");
        assert_eq!(
            re(&escape("<>:;,!%@/'`"), 0)
                .fullmatch("<>:;,!%@/'`")
                .map(|m| m.span0()),
            Some((0, 11))
        );
        assert_eq!(escape("\u{b}\u{c}\r"), "\\\u{b}\\\u{c}\\\r");
        assert_eq!(
            re(&escape("\u{b}\u{c}\r"), 0)
                .fullmatch("\u{b}\u{c}\r")
                .map(|m| m.span0()),
            Some((0, 3))
        );
        assert_eq!(escape(""), "");
        assert_eq!(
            re(&escape(""), 0).fullmatch("").map(|m| m.span0()),
            Some((0, 0))
        );
        assert_eq!(
            re(&format!(r"(?i)\b{}\b", escape("A.B")), 0)
                .search("x a.b y")
                .map(|m| m.span0()),
            Some((2, 5))
        );
    }

    #[test]
    fn named_groups_and_lastgroup() {
        {
            let m = re("(?P<a>x(y)?)|(?P<b>z)|(?P<c>w)?q", 0)
                .match_("xy")
                .unwrap();
            assert_eq!(
                (
                    m.lastgroup(),
                    m.named("a"),
                    m.named("b"),
                    m.named("c"),
                    m.start(1),
                    m.end(2),
                    m.span(3)
                ),
                (Some("a"), Some("xy"), None, None, Some(0), Some(2), None)
            );
        }
        {
            let m = re("(?P<a>x(y)?)|(?P<b>z)|(?P<c>w)?q", 0)
                .match_("x")
                .unwrap();
            assert_eq!(
                (
                    m.lastgroup(),
                    m.named("a"),
                    m.named("b"),
                    m.named("c"),
                    m.start(1),
                    m.end(2),
                    m.span(3)
                ),
                (Some("a"), Some("x"), None, None, Some(0), None, None)
            );
        }
        {
            let m = re("(?P<a>x(y)?)|(?P<b>z)|(?P<c>w)?q", 0)
                .match_("z")
                .unwrap();
            assert_eq!(
                (
                    m.lastgroup(),
                    m.named("a"),
                    m.named("b"),
                    m.named("c"),
                    m.start(1),
                    m.end(2),
                    m.span(3)
                ),
                (Some("b"), None, Some("z"), None, None, None, Some((0, 1)))
            );
        }
        {
            let m = re("(?P<a>x(y)?)|(?P<b>z)|(?P<c>w)?q", 0)
                .match_("q")
                .unwrap();
            assert_eq!(
                (
                    m.lastgroup(),
                    m.named("a"),
                    m.named("b"),
                    m.named("c"),
                    m.start(1),
                    m.end(2),
                    m.span(3)
                ),
                (None, None, None, None, None, None, None)
            );
        }
        {
            let m = re("(?P<a>x(y)?)|(?P<b>z)|(?P<c>w)?q", 0)
                .match_("wq")
                .unwrap();
            assert_eq!(
                (
                    m.lastgroup(),
                    m.named("a"),
                    m.named("b"),
                    m.named("c"),
                    m.start(1),
                    m.end(2),
                    m.span(3)
                ),
                (Some("c"), None, None, Some("w"), None, None, None)
            );
        }
        assert_eq!(re("(?P<a>.+ is not defined\\.$)|(?P<b>^Invalid channel number\\.$)|(?P<c>Settling time -?\\d+ too (short|long))", 0).match_("x is not defined.").and_then(|m| m.lastgroup().map(str::to_owned)), Some("a".to_owned()));
        assert_eq!(re("(?P<a>.+ is not defined\\.$)|(?P<b>^Invalid channel number\\.$)|(?P<c>Settling time -?\\d+ too (short|long))", 0).match_("Invalid channel number.").and_then(|m| m.lastgroup().map(str::to_owned)), Some("b".to_owned()));
        assert_eq!(re("(?P<a>.+ is not defined\\.$)|(?P<b>^Invalid channel number\\.$)|(?P<c>Settling time -?\\d+ too (short|long))", 0).match_("Settling time -3 too long").and_then(|m| m.lastgroup().map(str::to_owned)), Some("c".to_owned()));
        assert_eq!(re("(?P<a>.+ is not defined\\.$)|(?P<b>^Invalid channel number\\.$)|(?P<c>Settling time -?\\d+ too (short|long))", 0).match_("zzz").and_then(|m| m.lastgroup().map(str::to_owned)), None);
    }

    /// Sets of chars 0..=255 that Python's `\w \d \s \W \S` match.
    #[test]
    fn latin1_classes() {
        let tables: [(&str, &str); 5] = [
            (
                "w",
                "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz\u{aa}\u{b2}\u{b3}\u{b5}\u{b9}\u{ba}\u{bc}\u{bd}\u{be}\u{c0}\u{c1}\u{c2}\u{c3}\u{c4}\u{c5}\u{c6}\u{c7}\u{c8}\u{c9}\u{ca}\u{cb}\u{cc}\u{cd}\u{ce}\u{cf}\u{d0}\u{d1}\u{d2}\u{d3}\u{d4}\u{d5}\u{d6}\u{d8}\u{d9}\u{da}\u{db}\u{dc}\u{dd}\u{de}\u{df}\u{e0}\u{e1}\u{e2}\u{e3}\u{e4}\u{e5}\u{e6}\u{e7}\u{e8}\u{e9}\u{ea}\u{eb}\u{ec}\u{ed}\u{ee}\u{ef}\u{f0}\u{f1}\u{f2}\u{f3}\u{f4}\u{f5}\u{f6}\u{f8}\u{f9}\u{fa}\u{fb}\u{fc}\u{fd}\u{fe}\u{ff}",
            ),
            ("d", "0123456789"),
            ("s", "\t\n\u{b}\u{c}\r\u{1c}\u{1d}\u{1e}\u{1f} \u{85}\u{a0}"),
            (
                "W",
                "\u{0}\u{1}\u{2}\u{3}\u{4}\u{5}\u{6}\u{7}\u{8}\t\n\u{b}\u{c}\r\u{e}\u{f}\u{10}\u{11}\u{12}\u{13}\u{14}\u{15}\u{16}\u{17}\u{18}\u{19}\u{1a}\u{1b}\u{1c}\u{1d}\u{1e}\u{1f} !\"#$%&'()*+,-./:;<=>?@[\\]^`{|}~\u{7f}\u{80}\u{81}\u{82}\u{83}\u{84}\u{85}\u{86}\u{87}\u{88}\u{89}\u{8a}\u{8b}\u{8c}\u{8d}\u{8e}\u{8f}\u{90}\u{91}\u{92}\u{93}\u{94}\u{95}\u{96}\u{97}\u{98}\u{99}\u{9a}\u{9b}\u{9c}\u{9d}\u{9e}\u{9f}\u{a0}\u{a1}\u{a2}\u{a3}\u{a4}\u{a5}\u{a6}\u{a7}\u{a8}\u{a9}\u{ab}\u{ac}\u{ad}\u{ae}\u{af}\u{b0}\u{b1}\u{b4}\u{b6}\u{b7}\u{b8}\u{bb}\u{bf}\u{d7}\u{f7}",
            ),
            (
                "S",
                "\u{0}\u{1}\u{2}\u{3}\u{4}\u{5}\u{6}\u{7}\u{8}\u{e}\u{f}\u{10}\u{11}\u{12}\u{13}\u{14}\u{15}\u{16}\u{17}\u{18}\u{19}\u{1a}\u{1b}!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\u{7f}\u{80}\u{81}\u{82}\u{83}\u{84}\u{86}\u{87}\u{88}\u{89}\u{8a}\u{8b}\u{8c}\u{8d}\u{8e}\u{8f}\u{90}\u{91}\u{92}\u{93}\u{94}\u{95}\u{96}\u{97}\u{98}\u{99}\u{9a}\u{9b}\u{9c}\u{9d}\u{9e}\u{9f}\u{a1}\u{a2}\u{a3}\u{a4}\u{a5}\u{a6}\u{a7}\u{a8}\u{a9}\u{aa}\u{ab}\u{ac}\u{ad}\u{ae}\u{af}\u{b0}\u{b1}\u{b2}\u{b3}\u{b4}\u{b5}\u{b6}\u{b7}\u{b8}\u{b9}\u{ba}\u{bb}\u{bc}\u{bd}\u{be}\u{bf}\u{c0}\u{c1}\u{c2}\u{c3}\u{c4}\u{c5}\u{c6}\u{c7}\u{c8}\u{c9}\u{ca}\u{cb}\u{cc}\u{cd}\u{ce}\u{cf}\u{d0}\u{d1}\u{d2}\u{d3}\u{d4}\u{d5}\u{d6}\u{d7}\u{d8}\u{d9}\u{da}\u{db}\u{dc}\u{dd}\u{de}\u{df}\u{e0}\u{e1}\u{e2}\u{e3}\u{e4}\u{e5}\u{e6}\u{e7}\u{e8}\u{e9}\u{ea}\u{eb}\u{ec}\u{ed}\u{ee}\u{ef}\u{f0}\u{f1}\u{f2}\u{f3}\u{f4}\u{f5}\u{f6}\u{f7}\u{f8}\u{f9}\u{fa}\u{fb}\u{fc}\u{fd}\u{fe}\u{ff}",
            ),
        ];
        for (name, expected) in tables {
            let r = re(&format!("\\{name}"), 0);
            let got: String = (0u32..256)
                .filter_map(char::from_u32)
                .filter(|c| r.fullmatch(&c.to_string()).is_some())
                .collect();
            assert_eq!(got, expected, "class \\{name}");
        }
    }

    #[test]
    fn usable_in_lazylock_static() {
        static R: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| Regex::new(r"\d+", 0));
        assert_eq!(R.search("ab12").map(|m| m.span0()), Some((2, 4)));
    }

    #[test]
    #[should_panic(expected = "does not compile")]
    fn bad_pattern_panics_with_pattern() {
        re("(", 0);
    }

    #[test]
    #[should_panic(expected = "(unclosed")]
    fn bad_pattern_names_pattern() {
        re("(unclosed", 0);
    }
}
