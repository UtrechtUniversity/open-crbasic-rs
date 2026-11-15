//! Port of `parser.py` lines 1–695: token stream, segment splitting, semicolon/unclosed-paren
//! normalization, and statement/expression helpers.

use std::fmt;
use std::sync::LazyLock;

use crate::profiles::Profile;
use crate::pyre::{self, Regex};
use crate::pystr;
use crate::syntax::{COMPOUND_ASSIGNMENT_OPS, Expr, If, Stmt, StmtRef, Token, TokenValue};

pub static _DIRECTIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*#\s*(If_no_remove|If|ElseIf|Else|EndIf|UnDef)\b(.*)$",
        pyre::I,
    )
});
pub static _THEN_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bThen\s*$", pyre::I));
pub static _LEADING_WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z_][A-Za-z0-9_]*\$?)", 0));

pub const _RELOPS: [&str; 6] = ["=", "<>", "<", ">", "<=", ">="];

pub const UNCLOSED_PAREN_EXPR: &str = "unclosed parenthesized expression";
pub const UNCLOSED_DIM_PREFIX: &str = "unclosed dim: ";

pub const SNMP_OID_REASON: &str = "dotted SNMP OID literal";
pub static _SNMP_OID_STATEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*SNMPVariable\s*\(\s*(?:[^,]+,\s*)?\d[\d.\s]*[,)]",
        pyre::I,
    )
});

/// Port of `parser.py::_ParseError`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

pub const CONST_NOMATCH: &str = "\x00N";
pub const CONST_INCOMPLETE: &str = "\x00I";
pub const _CONST_TYPE_WORDS: [&str; 5] = ["LONG", "FLOAT", "DOUBLE", "STRING", "BOOLEAN"];

/// Port of `parser.py::_TokStream`
#[derive(Debug, Clone)]
pub struct TokStream {
    pub toks: Vec<Token>,
    pub i: usize,
    pub seg_text: String,
}

impl TokStream {
    /// Port of `parser.py::_TokStream.__init__`
    pub fn new(toks: Vec<Token>, seg_text: impl Into<String>) -> Self {
        Self {
            toks,
            i: 0,
            seg_text: seg_text.into(),
        }
    }

    /// Port of `parser.py::_TokStream.peek`
    pub fn peek(&self, k: usize) -> &Token {
        let j = self.i + k;
        if j < self.toks.len() {
            &self.toks[j]
        } else {
            self.toks.last().expect("TokStream requires an EOF token")
        }
    }

    /// Port of `parser.py::_TokStream.next`
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Token {
        let t = self.peek(0).clone();
        if t.kind != "EOF" {
            self.i += 1;
        }
        t
    }

    /// Port of `parser.py::_TokStream.at_end`
    pub fn at_end(&self) -> bool {
        self.peek(0).kind == "EOF"
    }

    /// Port of `parser.py::_TokStream.remaining`
    pub fn remaining(&self) -> &[Token] {
        &self.toks[self.i..]
    }

    /// Port of `parser.py::_TokStream.is_op`
    pub fn is_op(&self, text: &str, k: usize) -> bool {
        let t = self.peek(k);
        t.kind == "OP" && t.text == text
    }

    /// Port of `parser.py::_TokStream.is_ident`
    pub fn is_ident(&self, upper: &str, k: usize) -> bool {
        let t = self.peek(k);
        t.kind == "IDENT" && t.upper == upper
    }

    /// Port of `parser.py::_TokStream.expect_op`
    pub fn expect_op(&mut self, text: &str) -> Result<Token, ParseError> {
        if self.is_op(text, 0) {
            return Ok(self.next());
        }
        let t = self.peek(0);
        let found = if t.text.is_empty() { &t.kind } else { &t.text };
        Err(ParseError(format!(
            "expected {}, found {}",
            pystr::repr_str(text),
            pystr::repr_str(found)
        )))
    }

    /// Port of `parser.py::_TokStream.expect_close`
    pub fn expect_close(&mut self, text: &str) -> Result<Token, ParseError> {
        if self.at_end() {
            return Ok(self.peek(0).clone());
        }
        self.expect_op(text)
    }

    /// Port of `parser.py::_TokStream.expect_ident`
    pub fn expect_ident(&mut self, upper: Option<&str>) -> Result<Token, ParseError> {
        let t = self.peek(0);
        if t.kind == "IDENT" && upper.is_none_or(|u| t.upper == u) {
            return Ok(self.next());
        }
        let want = upper.unwrap_or("an identifier");
        let found = if t.text.is_empty() { &t.kind } else { &t.text };
        Err(ParseError(format!(
            "expected {want}, found {}",
            pystr::repr_str(found)
        )))
    }
}

/// Port of `parser.py::_eof_like`
pub fn _eof_like(line: i64) -> Token {
    Token {
        kind: "EOF".to_string(),
        text: String::new(),
        upper: String::new(),
        col: 0,
        line,
        value: TokenValue::None,
    }
}

/// Port of `parser.py::_split_text_segments`
pub fn _split_text_segments(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut segments: Vec<String> = Vec::new();
    let mut current: String = String::new();
    let mut in_string = false;
    let mut i = 0;
    let n = chars.len();
    while i < n {
        let ch = chars[i];
        if in_string {
            current.push(ch);
            if ch == '"' {
                if i + 1 < n && chars[i + 1] == '"' {
                    current.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            current.push(ch);
            i += 1;
            continue;
        }
        if ch == ':' {
            segments.push(std::mem::take(&mut current));
            i += 1;
            continue;
        }
        current.push(ch);
        i += 1;
    }
    segments.push(current);
    segments
}

pub const _RAW_NAME_FIRST: [&str; 2] = ["DATATABLE", "CONSTTABLE"];

fn is_assign_op(text: &str) -> bool {
    text == "=" || COMPOUND_ASSIGNMENT_OPS.contains(&text)
}

/// Port of `parser.py::_is_assignment`
pub fn _is_assignment(toks: &[Token]) -> bool {
    if toks.is_empty() || toks[0].kind != "IDENT" {
        return false;
    }
    let mut i = 1usize;
    let mut depth = 0i64;
    while i < toks.len() {
        let t = &toks[i];
        if t.kind == "OP" && t.text == "(" {
            depth += 1;
        } else if t.kind == "OP" && t.text == ")" {
            depth -= 1;
        } else if depth == 0 {
            if t.kind == "OP" && is_assign_op(&t.text) {
                return true;
            }
            if !((t.kind == "OP" && t.text == ".")
                || (t.kind == "IDENT" && toks[i - 1].text == "."))
            {
                return false;
            }
        }
        i += 1;
    }
    false
}

fn is_semi(t: &Token) -> bool {
    t.kind == "OP" && t.text == ";"
}

/// Port of `parser.py::_drop_semicolons`
pub fn _drop_semicolons(tokens: Vec<Token>, profile: &Profile) -> Vec<Token> {
    let real_len = tokens
        .iter()
        .position(|t| t.kind == "EOF")
        .unwrap_or(tokens.len());
    let real = &tokens[..real_len];
    let eof = &tokens[real_len..];
    let mut body: Vec<Token> = Vec::with_capacity(real.len());
    let mut body_idx: Vec<usize> = Vec::with_capacity(real.len());
    for (idx, t) in real.iter().enumerate() {
        if !is_semi(t) {
            body.push(t.clone());
            body_idx.push(idx);
        }
    }
    if body.len() == real.len() || body.is_empty() || body[0].kind != "IDENT" {
        return tokens;
    }
    let head = body[0].upper.as_str();
    let trailing_only = body_idx.last() == Some(&(body.len() - 1));
    let first = body_idx[0] == 0;

    let glued = |word: &str| -> bool {
        real.windows(2).any(|w| {
            let (a, t) = (&w[0], &w[1]);
            is_semi(t)
                && a.kind == "IDENT"
                && a.upper == word
                && t.col == a.col + i64::try_from(pystr::len(&a.text)).unwrap_or(0)
        })
    };

    let ok: bool;
    if head == "NEXT" {
        ok = first && !glued("NEXT");
    } else if head == "ENDSELECT" {
        if first && real.len() > 1 && is_semi(&real[1]) && !glued("ENDSELECT") {
            let mut j = 1usize;
            while j + 1 < real.len() && is_semi(&real[j + 1]) && real[j + 1].col == real[j].col + 1
            {
                j += 1;
            }
            let mut out = Vec::with_capacity(1 + real.len() - (j + 1) + eof.len());
            out.push(real[0].clone());
            out.extend_from_slice(&real[j + 1..]);
            out.extend_from_slice(eof);
            return out;
        }
        ok = false;
    } else if (head == "CASE"
        && body.len() == 2
        && body[1].upper == "ELSE"
        && first
        && real.len() >= 2
        && body_idx[1] == 1)
        || (head == "ELSEIF"
            && body.last().is_some_and(|t| t.upper == "THEN")
            && trailing_only
            && first
            && body.len() > 2
            && !glued("THEN"))
    {
        ok = true;
    } else if head == "SUB" || head == "FUNCTION" {
        ok = trailing_only && first && body.len() > 1;
    } else if profile.name != "cr6" && profile.name != "cr1000x" {
        ok = false;
    } else if head == "IF" || head == "ELSEIF" {
        let then = body
            .iter()
            .enumerate()
            .rfind(|(_, t)| t.kind == "IDENT" && t.upper == "THEN")
            .map(|(i, _)| i);
        if let Some(then_idx) = then {
            if !first || glued("THEN") || glued("ELSE") {
                ok = false;
            } else if then_idx + 1 < body.len() {
                ok = _is_assignment(&body[then_idx + 1..]);
            } else {
                ok = head == "ELSEIF" || !real[body_idx[then_idx] + 1..].iter().any(is_semi);
            }
        } else {
            ok = false;
        }
    } else if head == "CONST" {
        let eq = real
            .iter()
            .position(|t| t.kind == "OP" && t.text == "=")
            .unwrap_or(real.len());
        ok = first && !real[..eq].iter().any(is_semi);
    } else if head == "FOR" || head == "WHILE" || head == "SELECT" {
        ok = first;
    } else if head == "DO" || head == "LOOP" || head == "CASE" {
        let case_else = head == "CASE" && body.len() == 2 && body[1].upper == "ELSE";
        ok = first && body.len() > 1 && !case_else && (head == "CASE" || body_idx[1] == 1);
    } else if profile.functions().contains(head)
        && !profile.instructions().contains(head)
        && trailing_only
        && first
        && body.len() > 1
        && body[1].text == "("
        && body.last().is_some_and(|t| t.text == ")")
    {
        ok = true;
    } else {
        let member = body.len() > 1 && body[1].kind == "OP" && body[1].text == ".";
        ok = (!profile.instructions().contains(head)
            || (member && (head == "PUBLIC" || head == "DIM")))
            && _is_assignment(&body);
    }
    if ok {
        body.extend_from_slice(eof);
        return body;
    }
    if (profile.name == "cr6" || profile.name == "cr1000x") && head != "SUB" && head != "FUNCTION" {
        let name_slot = if head == "DATATABLE"
            && real.len() > 3
            && real[1].text == "("
            && real[2].kind == "IDENT"
        {
            Some(3usize)
        } else {
            None
        };
        let mut depth = 0i64;
        let mut kept: Vec<Token> = Vec::with_capacity(real.len());
        for (i, t) in real.iter().enumerate() {
            if t.kind == "OP" && (t.text == "(" || t.text == ")") {
                depth += if t.text == "(" { 1 } else { -1 };
            }
            if !(is_semi(t) && depth > 0 && name_slot != Some(i)) {
                kept.push(t.clone());
            }
        }
        if kept.len() < real.len() {
            kept.extend_from_slice(eof);
            return kept;
        }
    }
    tokens
}

pub const _S1_CALLS: [&str; 4] = ["ABS", "ABSLONG", "ARRAYLENGTH", "TYPEOF"];
pub const _S2_CALLS: [&str; 2] = ["LINENUM", "SIGNATURE"];

fn is_s1_call(name: &str) -> bool {
    _S1_CALLS.contains(&name)
}

fn is_s2_call(name: &str) -> bool {
    _S2_CALLS.contains(&name)
}

fn is_self_closing_call(name: &str) -> bool {
    is_s1_call(name) || is_s2_call(name)
}

pub const _BIN_OPS: [&str; 13] = [
    "+", "-", "*", "/", "^", "&", "=", "<>", "<", ">", "<=", ">=", "\\",
];
pub const _BIN_WORDS: [&str; 6] = ["AND", "OR", "XOR", "MOD", "EQV", "IMP"];

/// Port of `parser.py::_match_paren`
pub fn _match_paren(toks: &[Token], i: usize) -> Option<usize> {
    let mut depth = 0i64;
    for (j, tok) in toks.iter().enumerate().skip(i) {
        if tok.kind == "OP" && tok.text == "(" {
            depth += 1;
        } else if tok.kind == "OP" && tok.text == ")" {
            depth -= 1;
            if depth == 0 {
                return Some(j);
            }
        }
    }
    None
}

/// Port of `parser.py::_is_binary`
pub fn _is_binary(t: &Token) -> bool {
    (t.kind == "OP" && _BIN_OPS.contains(&t.text.as_str()))
        || (t.kind == "IDENT" && _BIN_WORDS.contains(&t.upper.as_str()))
}

/// Port of `parser.py::_expr_end`
pub fn _expr_end(toks: &[Token], mut i: usize) -> Option<usize> {
    let n = toks.len();
    loop {
        while i < n
            && ((toks[i].kind == "OP" && (toks[i].text == "+" || toks[i].text == "-"))
                || (toks[i].kind == "IDENT" && toks[i].upper == "NOT"))
        {
            i += 1;
        }
        if i >= n {
            return None;
        }
        let t = &toks[i];
        if t.kind == "NUMBER" || t.kind == "STRING" {
            i += 1;
        } else if t.kind == "IDENT" {
            i += 1;
            while i < n {
                if toks[i].kind == "OP"
                    && toks[i].text == "."
                    && i + 1 < n
                    && toks[i + 1].kind == "IDENT"
                {
                    i += 2;
                } else if toks[i].kind == "OP" && toks[i].text == "(" {
                    let j = _match_paren(toks, i)?;
                    i = j + 1;
                } else {
                    break;
                }
            }
        } else if t.kind == "OP" && t.text == "(" {
            let j = _match_paren(toks, i)?;
            i = j + 1;
        } else {
            return None;
        }
        if i < n && _is_binary(&toks[i]) {
            i += 1;
            continue;
        }
        return Some(i);
    }
}

/// Port of `parser.py::_close_open_flat`
pub fn _close_open_flat(tokens: &[Token], _profile: &Profile) -> Option<Vec<Token>> {
    let real_len = tokens
        .iter()
        .position(|t| t.kind == "EOF")
        .unwrap_or(tokens.len());
    let real = &tokens[..real_len];
    let mut stack: Vec<usize> = Vec::new();
    let mut end = real.len();
    let head = if real.first().is_some_and(|t| t.kind == "IDENT") {
        real[0].upper.as_str()
    } else {
        ""
    };
    for (i, t) in real.iter().enumerate() {
        if t.kind == "OP" && t.text == "(" {
            stack.push(i);
        } else if t.kind == "OP" && t.text == ")" {
            stack.pop()?;
        } else if t.kind == "IDENT"
            && t.upper == "THEN"
            && !stack.is_empty()
            && (head == "IF" || head == "ELSEIF")
        {
            end = i;
            break;
        }
    }
    if stack.len() != 1 || ((head == "WHILE" || head == "LOOP") && end != real.len()) {
        return None;
    }
    let open_at = stack[0];
    let prev = if open_at > 0 {
        Some(&real[open_at - 1])
    } else {
        None
    };
    let prev_ok = prev.is_some_and(|p| p.kind == "IDENT" && is_self_closing_call(&p.upper));
    if !prev_ok || open_at + 1 >= end || (open_at > 1 && real[open_at - 2].text == ".") {
        return None;
    }
    if real[open_at + 1..end]
        .iter()
        .any(|t| t.kind == "OP" && matches!(t.text.as_str(), "," | "{" | "}"))
        || real[open_at + 1..end]
            .iter()
            .any(|t| t.kind == "IDENT" && t.upper == "THEN")
    {
        return None;
    }
    let close = Token {
        kind: "OP".to_string(),
        text: ")".to_string(),
        upper: ")".to_string(),
        col: real[end - 1].col + i64::try_from(pystr::len(&real[end - 1].text)).unwrap_or(0),
        line: real[end - 1].line,
        value: TokenValue::None,
    };
    let mut out = Vec::with_capacity(tokens.len() + 1);
    out.extend_from_slice(&real[..end]);
    out.push(close);
    out.extend_from_slice(&real[end..]);
    out.extend_from_slice(&tokens[real_len..]);
    Some(out)
}

/// Port of `parser.py::_Closed`
#[derive(Debug, Clone)]
pub struct Closed {
    pub tokens: Vec<Token>,
    pub dropped: Option<Vec<Token>>,
}

/// Port of `parser.py::_close_open_call`
pub fn _close_open_call(tokens: Vec<Token>, profile: &Profile) -> Closed {
    let real_len = tokens
        .iter()
        .position(|t| t.kind == "EOF")
        .unwrap_or(tokens.len());
    let real = &tokens[..real_len];
    let mut cut_tokens: Option<Vec<Token>> = None;
    if !real.is_empty()
        && real[0].kind == "IDENT"
        && (real[0].upper == "WHILE"
            || ((real[0].upper == "DO" || real[0].upper == "LOOP")
                && real.len() > 1
                && real[1].kind == "IDENT"
                && (real[1].upper == "WHILE" || real[1].upper == "UNTIL")))
        && let Some(cut) = real
            .iter()
            .position(|t| t.kind == "IDENT" && t.upper == "THEN")
    {
        let mut t = Vec::with_capacity(cut + tokens.len() - real_len);
        t.extend_from_slice(&real[..cut]);
        t.extend_from_slice(&tokens[real_len..]);
        cut_tokens = Some(t);
    }
    let active = cut_tokens.as_deref().unwrap_or(&tokens);
    if let Some(out) = _close_open_extra(active, profile) {
        return out;
    }
    if let Some(out) = _close_open_flat(active, profile) {
        return Closed {
            tokens: out,
            dropped: None,
        };
    }
    if let Some(cut_toks) = cut_tokens {
        let depth: i64 = cut_toks
            .iter()
            .filter(|t| t.kind == "OP")
            .map(|t| match t.text.as_str() {
                "(" => 1,
                ")" => -1,
                _ => 0,
            })
            .sum();
        if depth > 0 {
            return Closed {
                tokens,
                dropped: None,
            };
        }
        return Closed {
            tokens: cut_toks,
            dropped: None,
        };
    }
    Closed {
        tokens,
        dropped: None,
    }
}

/// Port of `parser.py::_close_open_extra`
pub fn _close_open_extra(tokens: &[Token], profile: &Profile) -> Option<Closed> {
    let real_len = tokens
        .iter()
        .position(|t| t.kind == "EOF")
        .unwrap_or(tokens.len());
    let real = &tokens[..real_len];
    let eof = &tokens[real_len..];
    let head = if real.first().is_some_and(|t| t.kind == "IDENT") {
        real[0].upper.as_str()
    } else {
        ""
    };
    let mut stack: Vec<usize> = Vec::new();
    let mut end = real.len();
    for (i, t) in real.iter().enumerate() {
        if t.kind == "OP" && t.text == "(" {
            stack.push(i);
        } else if t.kind == "OP" && t.text == ")" {
            stack.pop()?;
        } else if t.kind == "IDENT"
            && t.upper == "THEN"
            && !stack.is_empty()
            && (head == "IF" || head == "ELSEIF")
        {
            end = i;
            break;
        }
    }
    if stack.is_empty()
        || real[..end]
            .iter()
            .any(|t| t.kind == "OP" && (t.text == "{" || t.text == "}"))
    {
        return None;
    }
    let last = &real[end - 1];
    let mk = |kind: &str, text: &str, value: TokenValue| Token {
        kind: kind.to_string(),
        text: text.to_string(),
        upper: text.to_string(),
        col: last.col + i64::try_from(pystr::len(&last.text)).unwrap_or(0),
        line: last.line,
        value,
    };
    let call_name = |o: usize| -> &str {
        if o > 0 && real[o - 1].kind == "IDENT" && !(o > 1 && real[o - 2].text == ".") {
            real[o - 1].upper.as_str()
        } else {
            ""
        }
    };
    let names: Vec<&str> = stack.iter().map(|&o| call_name(o)).collect();
    let n = stack.len();
    let inner = stack[n - 1];
    let content = &real[inner + 1..end];
    if (head == "IF" || head == "ELSEIF")
        && real[..end]
            .iter()
            .any(|t| t.kind == "IDENT" && t.upper == "THEN")
    {
        return None;
    }
    let closed = |body: &[Token]| -> Vec<Token> {
        let mut out =
            Vec::with_capacity(inner + 1 + body.len() + n + (real.len() - end) + eof.len());
        out.extend_from_slice(&real[..=inner]);
        out.extend_from_slice(body);
        for _ in 0..n {
            out.push(mk("OP", ")", TokenValue::None));
        }
        out.extend_from_slice(&real[end..]);
        out.extend_from_slice(eof);
        out
    };

    // ---- calls of the self-closing family, possibly nested in one another ----
    if names[..n - 1].iter().all(|nm| is_s1_call(nm)) && is_self_closing_call(names[n - 1]) {
        for k in 1..n {
            let prefix = &real[stack[k - 1] + 1..stack[k] - 1];
            if prefix.iter().any(|t| t.kind == "OP" && t.text == ",")
                || (!prefix.is_empty() && !_is_binary(&prefix[prefix.len() - 1]))
            {
                return None;
            }
        }
        if is_s1_call(names[n - 1]) && !content.is_empty() {
            let mut exprs: Vec<&[Token]> = Vec::new();
            let mut tail = content;
            while !tail.is_empty() && tail[0].text != "," {
                let e = _expr_end(tail, 0)?;
                if e == 0 {
                    return None;
                }
                exprs.push(&tail[..e]);
                tail = &tail[e..];
            }
            let comma = tail.len() == 1 && tail[0].text == ",";
            let max_exprs = if comma { n } else { n + 1 };
            if (!tail.is_empty() && !comma) || exprs.len() > max_exprs || exprs.is_empty() {
                return None;
            }
            if exprs.len() == 1 && !comma {
                return if n > 1 {
                    Some(Closed {
                        tokens: closed(content),
                        dropped: None,
                    })
                } else {
                    None
                };
            }
            let dropped: Vec<Token> = exprs[1..]
                .iter()
                .flat_map(|part| part.iter().cloned())
                .collect();
            if exprs[1..]
                .iter()
                .any(|part| part[0].kind == "IDENT" && part.len() > 1 && part[1].text == "(")
            {
                return None;
            }
            return Some(Closed {
                tokens: closed(exprs[0]),
                dropped: Some(dropped),
            });
        }
        if is_s2_call(names[n - 1])
            && (content.is_empty() || (n == 1 && content.len() == 1 && content[0].text == ","))
            && !real[..inner - 1]
                .iter()
                .any(|t| t.kind == "IDENT" && is_s2_call(&t.upper))
        {
            return Some(Closed {
                tokens: closed(&[mk("NUMBER", "0", TokenValue::Int(0))]),
                dropped: None,
            });
        }
        if is_s2_call(names[n - 1]) && n > 1 && !content.is_empty() {
            let e1 = _expr_end(content, 0);
            if e1 == Some(content.len()) && !content.iter().any(|t| t.text == ",") {
                return Some(Closed {
                    tokens: closed(content),
                    dropped: None,
                });
            }
        }
        return None;
    }

    // ---- `Y = A(LineNum(X) + 1`: a closed LineNum/Signature call inside one other open group ----
    let simple_assign =
        real.len() > 2 && real[0].kind == "IDENT" && real[1].kind == "OP" && real[1].text == "=";
    if n == 1
        && simple_assign
        && !profile.instructions().contains(head)
        && end == real.len()
        && !is_self_closing_call(names[0])
    {
        if content.iter().any(|t| t.text == ",")
            || real[..inner - 1]
                .iter()
                .any(|t| t.kind == "IDENT" && is_s2_call(&t.upper))
        {
            return None;
        }
        let calls: Vec<usize> = content
            .iter()
            .enumerate()
            .filter(|(_, t)| t.kind == "IDENT" && is_s2_call(&t.upper))
            .map(|(i, _)| i)
            .collect();
        if calls.len() != 1 || calls[0] + 1 >= content.len() || content[calls[0] + 1].text != "(" {
            return None;
        }
        let c = calls[0];
        let close = _match_paren(content, c + 1)?;
        let pre = &content[..c];
        let post = &content[close + 1..];
        if !pre.is_empty() && !_is_binary(&pre[pre.len() - 1]) {
            return None;
        }
        if !(post.is_empty() || _is_binary(&post[0]) && _expr_end(post, 1) == Some(post.len())) {
            return None;
        }
        let mut out = Vec::with_capacity(tokens.len() + 1);
        out.extend_from_slice(&real[..end]);
        out.push(mk("OP", ")", TokenValue::None));
        out.extend_from_slice(&real[end..]);
        out.extend_from_slice(eof);
        return Some(Closed {
            tokens: out,
            dropped: None,
        });
    }

    // ---- statement `A(LineNum(X) = 1`: the call closes the index too ----
    if n == 1
        && stack[0] == 1
        && real.len() > 5
        && real[0].kind == "IDENT"
        && real[2].kind == "IDENT"
        && is_s2_call(&real[2].upper)
        && real[3].text == "("
        && !profile.instructions().contains(head)
    {
        let close = _match_paren(real, 3)?;
        if close + 1 < real.len()
            && real[close + 1].kind == "OP"
            && real[close + 1].text == "="
            && close + 2 < real.len()
        {
            let one = Token {
                kind: "NUMBER".to_string(),
                text: "1".to_string(),
                upper: "1".to_string(),
                col: real[2].col,
                line: real[2].line,
                value: TokenValue::Int(1),
            };
            let paren = Token {
                kind: "OP".to_string(),
                text: ")".to_string(),
                upper: ")".to_string(),
                col: real[2].col,
                line: real[2].line,
                value: TokenValue::None,
            };
            let mut out = Vec::with_capacity(2 + 2 + (real.len() - (close + 1)) + eof.len());
            out.extend_from_slice(&real[..2]);
            out.push(one);
            out.push(paren);
            out.extend_from_slice(&real[close + 1..]);
            out.extend_from_slice(eof);
            return Some(Closed {
                tokens: out,
                dropped: None,
            });
        }
    }
    None
}

pub const _INLINE_BLOCK_WORDS: [&str; 8] = [
    "SELECT",
    "WHILE",
    "NEXT",
    "ENDSELECT",
    "WEND",
    "SUBSCAN",
    "NEXTSUBSCAN",
    "NEXTSCAN",
];

/// Port of `parser.py::_is_block_statement`
pub fn _is_block_statement(st: &Stmt) -> bool {
    match st {
        Stmt::DoLoop(_) | Stmt::For(_) => true,
        Stmt::Instruction(instr) => {
            let word = pystr::upper(&instr.keyword);
            _INLINE_BLOCK_WORDS.contains(&word.as_str())
                || (word == "END"
                    && matches!(
                        instr.args.first(),
                        Some(Some(arg)) if matches!(&**arg, Expr::Name(n) if pystr::upper(&n.name) == "SELECT")
                    ))
        }
        _ => false,
    }
}

/// Port of `parser.py::_hoist_block_statements`
pub fn _hoist_block_statements(stmt: &mut If) -> Vec<StmtRef> {
    let mut hoisted: Vec<StmtRef> = Vec::new();
    for body in [&mut stmt.inline, &mut stmt.inline_else]
        .into_iter()
        .flatten()
    {
        let len = body.len();
        for (position, st) in body.iter().enumerate() {
            if _is_block_statement(st) {
                st.base().hoisted_last.set(Some(position == len - 1));
                hoisted.push(st.clone());
            }
        }
        body.retain(|st| !_is_block_statement(st));
    }
    let origin = stmt.base.origin.borrow().clone();
    if origin.is_some() {
        for st in &hoisted {
            (*st.base().origin.borrow_mut()).clone_from(&origin);
        }
    }
    hoisted
}

/// Port of `parser.py::_split_colon_segments`
pub fn _split_colon_segments(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut segments: Vec<Vec<Token>> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let eof = tokens.last().expect("tokens must end with EOF").clone();
    for t in &tokens[..tokens.len().saturating_sub(1)] {
        if t.kind == "OP" && t.text == ":" {
            segments.push(std::mem::take(&mut current));
            continue;
        }
        current.push(t.clone());
    }
    segments.push(current);
    segments
        .into_iter()
        .map(|mut seg| {
            seg.push(eof.clone());
            seg
        })
        .collect()
}

/// Port of `parser.py::_CondFrame`
#[derive(Debug, Clone, Copy)]
pub struct CondFrame {
    pub any_true: bool,
    pub current: bool,
    pub parent_ok: bool,
}

pub const _WORD_BINARY_OPS: [&str; 6] = ["MOD", "AND", "OR", "XOR", "EQV", "IMP"];
pub static _FUNCTION_DEF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*Function\s+([A-Za-z_][A-Za-z0-9_]*)", pyre::I));

pub const _LOGICAL_LEAD_OPS: [&str; 15] = [
    "+", "-", "*", "/", "\\", "^", "&", "<", ">", "<=", ">=", "<>", "=<", "=>", "><",
];

pub const LOGICAL_ASSIGN: &str = "logical-prefixed assignment";

/// Port of `parser.py::_has_assign`
pub fn _has_assign(expr: &Expr) -> bool {
    match expr {
        Expr::Binary(b) => {
            b.op == "ASSIGN"
                || b.left.as_deref().is_some_and(_has_assign)
                || b.right.as_deref().is_some_and(_has_assign)
        }
        _ => false,
    }
}

pub static _DIGIT_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\d.][\w.]*", 0));

pub const _DEFERRED_SOURCE_INSTRUCTIONS: [&str; 9] = [
    "SAMPLE",
    "AVERAGE",
    "MAXIMUM",
    "MINIMUM",
    "TOTALIZE",
    "MEDIAN",
    "STDDEV",
    "MOMENT",
    "COVARIANCE",
];

/// Port of `parser.py::_dims_after_type_scan`
pub fn _dims_after_type_scan(text: &str, open_col: usize, close_col: usize) -> Option<usize> {
    let first_close = pystr::find(
        text,
        ")",
        Some(i64::try_from(open_col + 1).ok()?),
        Some(i64::try_from(close_col + 1).ok()?),
    )?;
    let chars: Vec<char> = text.chars().collect();
    let mut delims: Vec<usize> = Vec::new();
    let mut depth: i64 = 0;
    for (i, &c) in chars.iter().enumerate().take(close_col).skip(open_col + 1) {
        if c == '(' || c == '{' {
            depth += 1;
        } else if c == ')' || c == '}' {
            depth -= 1;
        } else if c == ',' && depth == 0 {
            delims.push(i);
        }
    }
    delims.push(close_col);
    if !(delims[0] < first_close && first_close < close_col) {
        return None;
    }
    let mut i = first_close + 1;
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    while i < chars.len() && chars[i] != ' ' && chars[i] != '\t' {
        i += 1;
    }
    if i >= close_col {
        return None;
    }
    let kept = delims.iter().filter(|&&d| d < i).count();
    let start = if kept > 0 {
        delims[kept - 1] + 1
    } else {
        open_col + 1
    };
    if pystr::strip(pystr::slice(
        text,
        Some(i64::try_from(start).ok()?),
        Some(i64::try_from(i).ok()?),
    ))
    .is_empty()
    {
        Some(kept)
    } else {
        None
    }
}

/// Port of `parser.py::_raw_word`
pub fn _raw_word(p: &TokStream, delims: &str) -> String {
    let col = usize::try_from(p.peek(0).col).unwrap_or(0);
    let chars: Vec<char> = p.seg_text.chars().collect();
    let mut end = col + 1;
    while end < chars.len() {
        let ch = chars[end];
        if ch == '\t' || delims.contains(ch) {
            break;
        }
        end += 1;
    }
    pystr::rstrip(pystr::slice(
        &p.seg_text,
        Some(i64::try_from(col).unwrap_or(0)),
        Some(i64::try_from(end).unwrap_or(i64::MAX)),
    ))
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;
    use crate::syntax::{Node, StmtBase};

    fn tok(kind: &str, text: &str, col: i64) -> Token {
        Token {
            kind: kind.to_string(),
            text: text.to_string(),
            upper: pystr::upper(text),
            col,
            line: 1,
            value: TokenValue::None,
        }
    }

    fn eof() -> Token {
        _eof_like(1)
    }

    #[test]
    fn split_text_segments_respects_strings_not_parens() {
        assert_eq!(
            _split_text_segments("A = 1 : B = \"x:y\" : Foo(a:b)"),
            vec!["A = 1 ", " B = \"x:y\" ", " Foo(a", "b)"]
        );
        assert_eq!(
            _split_text_segments("S = \"a\"\":b\""),
            vec!["S = \"a\"\":b\""]
        );
    }

    #[test]
    fn is_assignment_matches_python() {
        let toks = vec![
            tok("IDENT", "A", 0),
            tok("OP", "(", 1),
            tok("NUMBER", "1", 2),
            tok("OP", ")", 3),
            tok("OP", "=", 5),
            tok("NUMBER", "2", 7),
            eof(),
        ];
        assert!(_is_assignment(&toks));
        let non = vec![
            tok("IDENT", "Delay", 0),
            tok("OP", "(", 5),
            tok("NUMBER", "1", 6),
            tok("OP", ")", 7),
            eof(),
        ];
        assert!(!_is_assignment(&non));
    }

    #[test]
    fn drop_semicolons_handles_cr6_and_cr300() {
        let cr6 = profiles::get("cr6").unwrap();
        let cr300 = profiles::get("cr300").unwrap();
        // `X = 1 ;`
        let toks = vec![
            tok("IDENT", "X", 0),
            tok("OP", "=", 2),
            tok("NUMBER", "1", 4),
            tok("OP", ";", 6),
            eof(),
        ];
        assert_eq!(_drop_semicolons(toks.clone(), cr6).len(), 4);
        assert_eq!(_drop_semicolons(toks, cr300).len(), 5);
        // `Next Y ;`
        let nxt = vec![
            tok("IDENT", "Next", 0),
            tok("IDENT", "Y", 5),
            tok("OP", ";", 7),
            eof(),
        ];
        assert_eq!(_drop_semicolons(nxt, cr300).len(), 3);
    }

    #[test]
    fn close_open_call_closes_abs() {
        let cr6 = profiles::get("cr6").unwrap();
        let toks = vec![
            tok("IDENT", "X", 0),
            tok("OP", "=", 2),
            tok("IDENT", "Abs", 4),
            tok("OP", "(", 7),
            tok("IDENT", "Y", 8),
            eof(),
        ];
        let closed = _close_open_call(toks, cr6);
        assert_eq!(closed.tokens.len(), 7);
        assert_eq!(closed.tokens[5].text, ")");
        assert!(closed.dropped.is_none());
    }

    #[test]
    fn dims_after_type_scan_matches_python() {
        // `Dim A(2,(3), 4)`
        let s = "Dim A(2,(3), 4)";
        assert_eq!(_dims_after_type_scan(s, 5, 14), Some(2));
    }

    #[test]
    fn split_colon_segments_keeps_eof() {
        let toks = vec![
            tok("IDENT", "A", 0),
            tok("OP", ":", 2),
            tok("IDENT", "B", 4),
            eof(),
        ];
        let segs = _split_colon_segments(&toks);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].last().unwrap().kind, "EOF");
        assert_eq!(segs[1].last().unwrap().kind, "EOF");
    }

    #[test]
    fn hoist_block_statements_moves_do_out() {
        let mut if_stmt = If {
            base: StmtBase::new(1, "f"),
            cond: None,
            inline: Some(vec![std::rc::Rc::new(Stmt::DoLoop(
                crate::syntax::DoLoop {
                    base: StmtBase::new(1, "f"),
                    keyword: "DO".into(),
                    cond_kind: None,
                    cond: None,
                },
            ))]),
            inline_else: None,
        };
        let hoisted = _hoist_block_statements(&mut if_stmt);
        assert_eq!(hoisted.len(), 1);
        assert!(if_stmt.inline.unwrap().is_empty());
        assert_eq!(hoisted[0].base().hoisted_last.get(), Some(true));
    }
}
