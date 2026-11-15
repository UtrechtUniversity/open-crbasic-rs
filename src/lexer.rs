//! Port of `lexer.py`.

use std::fmt;
use std::sync::LazyLock;

use crate::{pyre, pystr};

const OPERATORS: &[&str] = &[
    "<>", "<=", ">=", "<<", ">>", "+=", "-=", "*=", "/=", "\\=", "^=", "&=", "+", "-", "*", "/",
    "\\", "^", "&", "=", "<", ">", "(", ")", ",", ".", ":", ";", "{", "}", "#", "!", "@",
];

static OP_RE: LazyLock<pyre::Regex> = LazyLock::new(|| {
    let mut ops: Vec<&str> = OPERATORS.to_vec();
    ops.sort_by_key(|op| std::cmp::Reverse(op.len()));
    let pat = ops
        .into_iter()
        .map(pyre::escape)
        .collect::<Vec<_>>()
        .join("|");
    pyre::Regex::new(&pat, 0)
});

static IDENT_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"[A-Za-z_][A-Za-z0-9_]*\$?", 0));
static HEX_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"&[Hh][0-9A-Fa-f]+", 0));
static BIN_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"&[Bb][01]+", 0));
static NUMBER_RE: LazyLock<pyre::Regex> =
    LazyLock::new(|| pyre::Regex::new(r"(\d+\.\d*|\.\d+|\d+)([Ee][+-]?\d+)?", 0));
static WORD_RE: LazyLock<pyre::Regex> = LazyLock::new(|| pyre::Regex::new(r"\w+", 0));

pub const KEYWORD_KINDS: &[&str] = &["IDENT"];

/// Port of `lexer.py::Token` value.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenValue {
    None,
    Int(i64),
    Float(f64),
    Str(String),
}

/// Port of `lexer.py::Token`.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: String,
    pub text: String,
    pub upper: String,
    pub col: usize,
    pub line: usize,
    pub value: TokenValue,
}

impl Token {
    fn new(
        kind: &str,
        text: impl Into<String>,
        upper: impl Into<String>,
        col: usize,
        line: usize,
        value: TokenValue,
    ) -> Self {
        Self {
            kind: kind.to_string(),
            text: text.into(),
            upper: upper.into(),
            col,
            line,
            value,
        }
    }
}

/// Port of `lexer.py::LexError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub col: usize,
}

impl LexError {
    /// Port of `lexer.py::LexError.__init__`.
    pub fn new(message: impl Into<String>, col: usize) -> Self {
        Self {
            message: message.into(),
            col,
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LexError {}

/// Port of `lexer.py::_number_value`.
#[allow(clippy::used_underscore_items)]
fn _number_value(text: &str) -> TokenValue {
    if pystr::char_at(text, 0) == Some('&') {
        let base = if matches!(pystr::char_at(text, 1), Some('H' | 'h')) {
            16
        } else {
            2
        };
        let digits = pystr::slice(text, Some(2), None);
        let n = pystr::parse_int_radix(digits, base)
            .unwrap_or_else(|| panic!("invalid radix-{base} integer literal: {text:?}"));
        return TokenValue::Int(n);
    }
    if text.contains('.') || text.contains('e') || text.contains('E') {
        let x =
            pystr::parse_float(text).unwrap_or_else(|| panic!("invalid float literal: {text:?}"));
        return TokenValue::Float(x);
    }
    let n = pystr::parse_int(text).unwrap_or_else(|| panic!("invalid int literal: {text:?}"));
    TokenValue::Int(n)
}

/// Port of `lexer.py::tokenize`.
pub fn tokenize(text: &str, line: usize) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        let ch = chars[i];
        if ch == ' ' || ch == '\t' {
            i += 1;
            continue;
        }
        if ch == '"' {
            let start = i;
            i += 1;
            let mut out = String::new();
            loop {
                if i >= n {
                    return Err(LexError::new("unterminated string", start));
                }
                if chars[i] == '"' {
                    if i + 1 < n && chars[i + 1] == '"' {
                        out.push('"');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                out.push(chars[i]);
                i += 1;
            }
            let raw: String = chars[start..i].iter().collect();
            tokens.push(Token::new(
                "STRING",
                raw.clone(),
                raw,
                start,
                line,
                TokenValue::Str(out),
            ));
            continue;
        }
        if let Some(m) = HEX_RE
            .match_at(text, i)
            .or_else(|| BIN_RE.match_at(text, i))
        {
            let raw = m.group0();
            tokens.push(Token::new(
                "NUMBER",
                raw,
                pystr::upper(raw),
                i,
                line,
                _number_value(raw),
            ));
            i = m.end0();
            continue;
        }
        if ch == '.'
            && i + 1 < n
            && pystr::isdigit(&chars[i + 1].to_string())
            && tokens.last().is_some_and(|t: &Token| t.kind == "IDENT")
            && i > 0
            && (pystr::isalnum(&chars[i - 1].to_string()) || chars[i - 1] == '_')
        {
            tokens.push(Token::new("OP", ".", ".", i, line, TokenValue::None));
            let m = WORD_RE
                .match_at(text, i + 1)
                .expect("a digit follows the `.`");
            let word = m.group0();
            tokens.push(Token::new(
                "IDENT",
                word,
                pystr::upper(word),
                i + 1,
                line,
                TokenValue::None,
            ));
            i = m.end0();
            continue;
        }
        if pystr::isdigit(&ch.to_string())
            || (ch == '.' && i + 1 < n && pystr::isdigit(&chars[i + 1].to_string()))
        {
            let m = NUMBER_RE
                .match_at(text, i)
                .expect("a digit, or `.` and a digit");
            let raw = m.group0();
            let value = _number_value(raw);
            let mut end = m.end0();
            if end < n && (chars[end] == 'R' || chars[end] == 'r') {
                let after = if end + 1 < n {
                    let next_ch = chars[end + 1];
                    pystr::isalnum(&next_ch.to_string()) || next_ch == '_'
                } else {
                    false
                };
                if !after {
                    end += 1;
                }
            }
            let slice: String = chars[i..end].iter().collect();
            tokens.push(Token::new("NUMBER", slice.clone(), slice, i, line, value));
            i = end;
            continue;
        }
        if let Some(m) = IDENT_RE.match_at(text, i) {
            let raw = m.group0();
            tokens.push(Token::new(
                "IDENT",
                raw,
                pystr::upper(raw),
                i,
                line,
                TokenValue::None,
            ));
            i = m.end0();
            continue;
        }
        if let Some(m) = OP_RE.match_at(text, i) {
            let raw = m.group0();
            tokens.push(Token::new("OP", raw, raw, i, line, TokenValue::None));
            i = m.end0();
            continue;
        }
        return Err(LexError::new(
            format!(
                "unrecognised character {}",
                pystr::repr_str(&ch.to_string())
            ),
            i,
        ));
    }
    tokens.push(Token::new("EOF", "", "", n, line, TokenValue::None));
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_decl_matches_python() {
        let toks = tokenize("Public x As Long = 42", 1).unwrap();
        assert_eq!(
            toks,
            vec![
                Token::new("IDENT", "Public", "PUBLIC", 0, 1, TokenValue::None),
                Token::new("IDENT", "x", "X", 7, 1, TokenValue::None),
                Token::new("IDENT", "As", "AS", 9, 1, TokenValue::None),
                Token::new("IDENT", "Long", "LONG", 12, 1, TokenValue::None),
                Token::new("OP", "=", "=", 17, 1, TokenValue::None),
                Token::new("NUMBER", "42", "42", 19, 1, TokenValue::Int(42)),
                Token::new("EOF", "", "", 21, 1, TokenValue::None),
            ]
        );
    }

    #[test]
    fn string_escaping_matches_python() {
        let toks = tokenize(r#"s = "hello ""world""""#, 2).unwrap();
        assert_eq!(
            toks,
            vec![
                Token::new("IDENT", "s", "S", 0, 2, TokenValue::None),
                Token::new("OP", "=", "=", 2, 2, TokenValue::None),
                Token::new(
                    "STRING",
                    r#""hello ""world""""#,
                    r#""hello ""world""""#,
                    4,
                    2,
                    TokenValue::Str(r#"hello "world""#.to_string()),
                ),
                Token::new("EOF", "", "", 21, 2, TokenValue::None),
            ]
        );
    }

    #[test]
    fn numbers_and_double_suffix_match_python() {
        let toks = tokenize("y = &HFF + &b1010 + .5 + 2.15R", 3).unwrap();
        assert_eq!(
            toks,
            vec![
                Token::new("IDENT", "y", "Y", 0, 3, TokenValue::None),
                Token::new("OP", "=", "=", 2, 3, TokenValue::None),
                Token::new("NUMBER", "&HFF", "&HFF", 4, 3, TokenValue::Int(255)),
                Token::new("OP", "+", "+", 9, 3, TokenValue::None),
                Token::new("NUMBER", "&b1010", "&B1010", 11, 3, TokenValue::Int(10)),
                Token::new("OP", "+", "+", 18, 3, TokenValue::None),
                Token::new("NUMBER", ".5", ".5", 20, 3, TokenValue::Float(0.5)),
                Token::new("OP", "+", "+", 23, 3, TokenValue::None),
                Token::new("NUMBER", "2.15R", "2.15R", 25, 3, TokenValue::Float(2.15)),
                Token::new("EOF", "", "", 30, 3, TokenValue::None),
            ]
        );
    }

    #[test]
    fn dotted_member_and_ident_kinds_match_python() {
        let toks = tokenize("Tmp.001 = _60Hz + CHR$(65)", 4).unwrap();
        assert_eq!(
            toks,
            vec![
                Token::new("IDENT", "Tmp", "TMP", 0, 4, TokenValue::None),
                Token::new("OP", ".", ".", 3, 4, TokenValue::None),
                Token::new("IDENT", "001", "001", 4, 4, TokenValue::None),
                Token::new("OP", "=", "=", 8, 4, TokenValue::None),
                Token::new("IDENT", "_60Hz", "_60HZ", 10, 4, TokenValue::None),
                Token::new("OP", "+", "+", 16, 4, TokenValue::None),
                Token::new("IDENT", "CHR$", "CHR$", 18, 4, TokenValue::None),
                Token::new("OP", "(", "(", 22, 4, TokenValue::None),
                Token::new("NUMBER", "65", "65", 23, 4, TokenValue::Int(65)),
                Token::new("OP", ")", ")", 25, 4, TokenValue::None),
                Token::new("EOF", "", "", 26, 4, TokenValue::None),
            ]
        );
    }

    #[test]
    fn multi_char_operators_match_python() {
        let toks = tokenize("a += 1 <> 2 <= 3 >= 4 << 1 >> 1", 5).unwrap();
        assert_eq!(
            toks,
            vec![
                Token::new("IDENT", "a", "A", 0, 5, TokenValue::None),
                Token::new("OP", "+=", "+=", 2, 5, TokenValue::None),
                Token::new("NUMBER", "1", "1", 5, 5, TokenValue::Int(1)),
                Token::new("OP", "<>", "<>", 7, 5, TokenValue::None),
                Token::new("NUMBER", "2", "2", 10, 5, TokenValue::Int(2)),
                Token::new("OP", "<=", "<=", 12, 5, TokenValue::None),
                Token::new("NUMBER", "3", "3", 15, 5, TokenValue::Int(3)),
                Token::new("OP", ">=", ">=", 17, 5, TokenValue::None),
                Token::new("NUMBER", "4", "4", 20, 5, TokenValue::Int(4)),
                Token::new("OP", "<<", "<<", 22, 5, TokenValue::None),
                Token::new("NUMBER", "1", "1", 25, 5, TokenValue::Int(1)),
                Token::new("OP", ">>", ">>", 27, 5, TokenValue::None),
                Token::new("NUMBER", "1", "1", 30, 5, TokenValue::Int(1)),
                Token::new("EOF", "", "", 31, 5, TokenValue::None),
            ]
        );
    }

    #[test]
    fn lex_errors() {
        let err = tokenize("\"unterminated", 1).unwrap_err();
        assert_eq!(err, LexError::new("unterminated string", 0));
        let err = tokenize("x ~ y", 1).unwrap_err();
        assert_eq!(err, LexError::new("unrecognised character '~'", 2));
    }
}
