//! Port of `parser.py` lines 2283–2901: `Const`, `Public`/`Dim`, `Alias`, and `Units` parsing.

use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval;
use crate::pyre::{self, Regex};
use crate::pystr;
use crate::syntax::{
    ConstDecl, ConstValue, Declaration, Expr, ExprRef, Instruction, Juxtaposed, Node, Number,
    NumberValue, Stmt, StmtBase, StringLit, Token, TokenValue, Unparsed, VarDecl,
};

use super::Parser;
use super::helpers::{
    _CONST_TYPE_WORDS, _dims_after_type_scan, _raw_word, CONST_INCOMPLETE, CONST_NOMATCH,
    ParseError, TokStream, UNCLOSED_DIM_PREFIX,
};
use super::tokenize_for_parser;

static _NON_SPACE_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\s,]+", 0));
static _CONST_LEAD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*Const\b", pyre::I));
static _CONST_BAD_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*(?:=\s*)*(\d\w*)(?![\w.])", 0));
static _CONST_UNTERM_NOEQ_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*Const\s+[A-Za-z_]\w*(\([^()]*\))?\s+", pyre::I));
static _CONST_UNTERM_SUB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\s*Const\s+([A-Za-z_]\w*)\(\s*([^\s,()]+(?:\s+[^\s,()]+)*)\s*[,)]",
        pyre::I,
    )
});
static _CONST_UNTERM_EMPTY_SUB_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*Const\s+[A-Za-z_]\w*\(\s*,*\s*\)\s*=", pyre::I));
static _DIGIT_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d\w*", 0));

enum ConstBail {
    Stmt(Box<Stmt>),
    Err(ParseError),
}

impl Parser {
    /// Port of `parser.py::_Parser._parse_const`
    pub fn _parse_const(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next();
        let mut eq_at: Option<usize> = None;
        let mut lead = false;
        if p.is_op("=", 0) {
            let j = p.i;
            while p.is_op("=", 0) {
                p.next();
            }
            let tok = p.peek(0).clone();
            let after = p.peek(1).clone();
            if tok.kind == "EOF" {
                return Ok(Stmt::Unparsed(Unparsed {
                    base: StmtBase::new(line, file),
                    reason: "Const without '='".to_string(),
                    expr: None,
                }));
            }
            if tok.kind == "IDENT" && !(after.kind == "IDENT" && after.upper == "AS") {
                lead = true;
            } else if tok.kind == "OP" && tok.text == "," {
                p.i = p.toks.len() - 1;
                return Ok(Stmt::Unparsed(Unparsed {
                    base: StmtBase::new(line, file),
                    reason: "Const lead: CONST needs assignment.".to_string(),
                    expr: None,
                }));
            } else if tok.kind == "STRING"
                || (tok.kind == "OP" && tok.text == "*")
                || (tok.kind == "NUMBER"
                    && !pystr::isdigit(&tok.text)
                    && !pystr::lstrip_chars(&pystr::upper(&tok.text), "&").contains('E'))
                || (tok.kind == "OP" && tok.text == "-" && after.kind == "NUMBER")
            {
                let (text, rest_at) = if tok.kind == "OP" && tok.text == "-" {
                    (format!("{}{}", tok.text, after.text), 2usize)
                } else {
                    (tok.text.clone(), 1usize)
                };
                let mut msgs = vec![format!("Invalid variable or constant name: {text}.")];
                if pystr::char_at(&text, 0).is_some_and(|c| c.is_ascii_digit()) {
                    msgs.push(format!("Const name {text} illegal, starts with a number."));
                } else if p.peek(rest_at).kind == "EOF" {
                    msgs.push("CONST needs assignment.".to_string());
                }
                p.i = p.toks.len() - 1;
                return Ok(Stmt::Unparsed(Unparsed {
                    base: StmtBase::new(line, file),
                    reason: format!("Const lead: {}", msgs.join("\n")),
                    expr: None,
                }));
            } else {
                p.i = j;
            }
        }

        let body_res: Result<(Token, Option<String>, ExprRef, (i64, i64)), ConstBail> = (|| {
            let name_tok = p.expect_ident(None).map_err(ConstBail::Err)?;
            let mut vtype: Option<String> = None;
            if !lead
                && p.is_op("(", 0)
                && p.peek(0).col
                    == name_tok.col + i64::try_from(pystr::len(&name_tok.text)).unwrap_or(0)
            {
                p.next();
                while p.is_op(",", 0) {
                    p.next();
                }
                if p.is_op(")", 0) {
                    p.next();
                } else {
                    let sub_i = p.i;
                    let dim_start_col = p.peek(0).col;
                    if p.is_op("=", 0) {
                        // `Const J( = 1` is `Const J = 1`
                    } else {
                        let Ok(arr_dim) = self._parse_expr(p, line, file) else {
                            return Err(match self._const_bad_subscript(p, sub_i, line, file) {
                                Ok(s) => ConstBail::Stmt(Box::new(s)),
                                Err(e) => ConstBail::Err(e),
                            });
                        };
                        if !(p.at_end() || p.is_op(",", 0) || p.is_op(")", 0))
                            && p.toks[sub_i..]
                                .iter()
                                .any(|t| t.kind == "OP" && t.text == "=")
                        {
                            return Err(
                                match self._const_sub_unparsed(
                                    p,
                                    sub_i,
                                    line,
                                    file,
                                    CONST_INCOMPLETE,
                                    false,
                                ) {
                                    Ok(s) => ConstBail::Stmt(Box::new(s)),
                                    Err(e) => ConstBail::Err(e),
                                },
                            );
                        }
                        let arr_span = (dim_start_col, p.peek(0).col);
                        if p.toks[sub_i..]
                            .iter()
                            .any(|t| t.kind == "OP" && t.text == "=")
                        {
                            return Err(ConstBail::Stmt(Box::new(
                                self._const_array(p, line, file, &name_tok, arr_dim, arr_span),
                            )));
                        }
                        return Err(ConstBail::Err(ParseError(
                            "Const subscript without '='".to_string(),
                        )));
                    }
                }
            }
            if p.is_ident("AS", 0) {
                p.next();
                let type_tok = p.expect_ident(None).map_err(ConstBail::Err)?;
                let mut vt = type_tok.upper.clone();
                let word = _NON_SPACE_COMMA_RE
                    .match_(pystr::slice(&p.seg_text, Some(type_tok.col), None))
                    .map_or("", |m| m.group0());
                if !_CONST_TYPE_WORDS.contains(&pystr::upper(word).as_str())
                    && p.toks.iter().any(|t| t.kind == "OP" && t.text == "=")
                {
                    p.i = p.toks.len() - 1;
                    let zero = Rc::new(Expr::Number(Number {
                        node: Node::new(line, file),
                        value: NumberValue::Int(0),
                        text: "0".to_string(),
                    }));
                    self.env
                        .define(&name_tok.text, ConstValue::Int(0), Some(false));
                    return Err(ConstBail::Stmt(Box::new(Stmt::ConstDecl(ConstDecl {
                        base: StmtBase::new(line, file),
                        name: name_tok.text,
                        value: Some(zero),
                        type_: Some(word.to_string()),
                        value_span: Some((0, 0)),
                    }))));
                }
                if p.is_op("!", 0) {
                    p.next();
                    vt.push('!');
                }
                if p.is_op("*", 0)
                    && p.remaining()
                        .iter()
                        .any(|t| t.kind == "OP" && t.text == "=")
                {
                    let star = p.peek(0).clone();
                    let mut depth = 0i64;
                    let mut end = p.toks.len() - 1;
                    for (j, t) in p.toks.iter().enumerate().take(p.toks.len() - 1).skip(p.i) {
                        if t.kind == "OP" {
                            if t.text == "(" {
                                depth += 1;
                            } else if t.text == ")" {
                                depth -= 1;
                            } else if t.text == "," && depth == 0 {
                                end = j;
                                break;
                            }
                        }
                    }
                    let stop = if p.toks[end].kind == "EOF" {
                        i64::try_from(pystr::len(&p.seg_text)).unwrap_or(0)
                    } else {
                        p.toks[end].col
                    };
                    p.i = p.toks.len() - 1;
                    let slice = pystr::slice(&p.seg_text, Some(star.col), Some(stop));
                    return Err(ConstBail::Stmt(Box::new(Stmt::Unparsed(Unparsed {
                        base: StmtBase::new(line, file),
                        reason: format!("Const bad value: {slice}"),
                        expr: None,
                    }))));
                }
                if vt == "STRING" && p.is_op("*", 0) {
                    p.next();
                    self._parse_expr(p, line, file).map_err(ConstBail::Err)?;
                }
                vtype = Some(vt);
            }
            if lead && p.at_end() {
                return Err(ConstBail::Stmt(Box::new(Stmt::Unparsed(Unparsed {
                    base: StmtBase::new(line, file),
                    reason: format!("Const lead: name {}", name_tok.text),
                    expr: None,
                }))));
            }
            if lead && p.is_op(",", 0) {
                p.next();
            } else if lead && !p.is_op("=", 0) {
                // `Const = X Y`
            } else if p.is_op("(", 0)
                && p.remaining()
                    .iter()
                    .any(|t| t.kind == "OP" && t.text == "=")
            {
                // `Const J (5) = 7`
            } else if !lead
                && p.peek(0).kind == "IDENT"
                && !p.is_ident("AS", 0)
                && p.remaining()
                    .iter()
                    .any(|t| t.kind == "OP" && t.text == "=")
            {
                let rest_col = p.peek(0).col;
                p.i = p.toks.len() - 1;
                let slice = pystr::slice(&p.seg_text, Some(rest_col), None);
                return Err(ConstBail::Stmt(Box::new(Stmt::Unparsed(Unparsed {
                    base: StmtBase::new(line, file),
                    reason: format!("Const bad value: {slice}"),
                    expr: None,
                }))));
            } else {
                p.expect_op("=").map_err(ConstBail::Err)?;
            }
            eq_at = Some(p.i);
            let value_start_col = p.peek(0).col;
            let outer = self.to_ok;
            self.to_ok = true;
            let value_res = self._parse_expr(p, line, file);
            self.to_ok = outer;
            let value = value_res.map_err(ConstBail::Err)?;
            let value_span = (value_start_col, p.peek(0).col);
            if p.is_op(",", 0) || p.is_op(")", 0) || p.is_op("}", 0) {
                p.i = p.toks.len() - 1;
            } else if !p.at_end() {
                let eq = eq_at.unwrap();
                let mut rest_len = 0usize;
                let mut d = 0i64;
                for t in &p.toks[eq..p.toks.len() - 1] {
                    if t.kind == "OP" {
                        if t.text == "," && d <= 0 {
                            break;
                        }
                        if t.text == "(" {
                            d += 1;
                        } else if t.text == ")" {
                            d -= 1;
                        }
                    }
                    rest_len += 1;
                }
                let stop = if eq + rest_len < p.toks.len() - 1 {
                    p.toks[eq + rest_len].col
                } else {
                    i64::try_from(pystr::len(&p.seg_text)).unwrap_or(0)
                };
                let text = pystr::slice(&p.seg_text, Some(value_start_col), Some(stop));
                p.i = p.toks.len() - 1;
                let prefix = if d > 0 {
                    CONST_NOMATCH
                } else {
                    CONST_INCOMPLETE
                };
                return Err(ConstBail::Stmt(Box::new(Stmt::Unparsed(Unparsed {
                    base: StmtBase::new(line, file),
                    reason: format!("Const bad value: {prefix}{text}"),
                    expr: None,
                }))));
            }
            Ok((name_tok, vtype, value, value_span))
        })(
        );

        let (name_tok, vtype, value, value_span) = match body_res {
            Ok(ok) => ok,
            Err(ConstBail::Stmt(early_stmt)) => return Ok(*early_stmt),
            Err(ConstBail::Err(_)) => {
                if let Some(eq) = eq_at {
                    let rest = &p.toks[eq..];
                    if rest[0].kind == "EOF"
                        || (rest.len() >= 2
                            && rest[rest.len() - 2].kind == "OP"
                            && matches!(
                                rest[rest.len() - 2].text.as_str(),
                                "+" | "-" | "*" | "/" | "^" | "="
                            )
                            && pystr::len(&rest[rest.len() - 2].text) == 1)
                    {
                        let value = if rest[0].kind == "EOF" {
                            ""
                        } else {
                            pystr::slice(&p.seg_text, Some(rest[0].col), None)
                        };
                        return Ok(Stmt::Unparsed(Unparsed {
                            base: StmtBase::new(line, file),
                            reason: format!("Const bad value: {value}"),
                            expr: None,
                        }));
                    }
                    let depth: i64 = rest
                        .iter()
                        .filter(|t| t.kind == "OP")
                        .map(|t| match t.text.as_str() {
                            "(" => 1,
                            ")" => -1,
                            _ => 0,
                        })
                        .sum();
                    if depth >= 0 {
                        let value = pystr::slice(&p.seg_text, Some(rest[0].col), None).to_string();
                        p.i = p.toks.len() - 1;
                        let prefix = if depth > 0 { CONST_NOMATCH } else { "" };
                        return Ok(Stmt::Unparsed(Unparsed {
                            base: StmtBase::new(line, file),
                            reason: format!("Const bad value: {prefix}{value}"),
                            expr: None,
                        }));
                    }
                }
                let body = _CONST_LEAD_RE.sub_n("", &p.seg_text, 1);
                if let Some(bad) = _CONST_BAD_NAME_RE.match_(&body)
                    && body.contains('=')
                {
                    p.i = p.toks.len() - 1;
                    return Ok(Stmt::Unparsed(Unparsed {
                        base: StmtBase::new(line, file),
                        reason: format!("Const bad name: {}", bad.group(1).unwrap_or("")),
                        expr: None,
                    }));
                }
                if !body.contains('=') || pystr::strip(&body) == "=" {
                    p.i = p.toks.len() - 1;
                }
                return Ok(Stmt::Unparsed(Unparsed {
                    base: StmtBase::new(line, file),
                    reason: "Const without '='".to_string(),
                    expr: None,
                }));
            }
        };

        if let Ok(val) = consteval::value_of(Some(&value), &self.env) {
            let is_f = consteval::is_float(Some(&value), &self.env);
            self.env.define(&name_tok.text, val, is_f);
        }
        Ok(Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(line, file),
            name: name_tok.text,
            value: Some(value),
            type_: vtype,
            value_span: Some(value_span),
        }))
    }

    /// Port of `parser.py::_Parser._const_unterminated`
    pub fn _const_unterminated(
        &mut self,
        seg: &str,
        q: usize,
        line: i64,
        file: &str,
    ) -> Option<Stmt> {
        let pre = pystr::slice(seg, None, Some(i64::try_from(q).unwrap_or(0)));
        if !seg.contains('=') {
            if _CONST_UNTERM_NOEQ_RE.fullmatch(pre).is_some() {
                return Some(self._parse_text_segment(pystr::rstrip(pre), line, file));
            }
            return None;
        }
        if let Some(m) = _CONST_UNTERM_SUB_RE.match_(pre) {
            let synth = format!(
                "Const {}({}) = 0",
                m.group(1).unwrap_or(""),
                m.group(2).unwrap_or("")
            );
            return Some(self._parse_text_segment(&synth, line, file));
        }
        if _CONST_UNTERM_EMPTY_SUB_RE.match_(pre).is_some() {
            let quote_tail = pystr::slice(seg, Some(i64::try_from(q).unwrap_or(0)), None);
            return Some(Stmt::Unparsed(Unparsed {
                base: StmtBase::new(line, file),
                reason: format!(
                    "Const lead: String without end quote\nInvalid, or out of place expression: {quote_tail}."
                ),
                expr: None,
            }));
        }
        if let Some(eq) = pystr::find(pre, "=", None, None) {
            let after_eq = pystr::slice(pre, Some(i64::try_from(eq).unwrap_or(0)), None);
            if !pre.contains('(')
                && let Some(comma_off) = pystr::find(after_eq, ",", None, None)
            {
                let cut = i64::try_from(eq + comma_off).unwrap_or(0);
                return Some(self._parse_text_segment(
                    pystr::slice(seg, None, Some(cut)),
                    line,
                    file,
                ));
            }
        }
        Some(Stmt::Unparsed(Unparsed {
            base: StmtBase::new(line, file),
            reason: "Const lead: CONST needs assignment.".to_string(),
            expr: None,
        }))
    }

    /// Port of `parser.py::_Parser._const_bad_subscript`
    pub fn _const_bad_subscript(
        &mut self,
        p: &mut TokStream,
        start_i: usize,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        self._const_sub_unparsed(p, start_i, line, file, "", true)
    }

    /// Port of `parser.py::_Parser._const_sub_unparsed`
    pub fn _const_sub_unparsed(
        &mut self,
        p: &mut TokStream,
        start_i: usize,
        line: i64,
        file: &str,
        kind: &str,
        need_op: bool,
    ) -> Result<Stmt, ParseError> {
        let mut depth = 0i64;
        let mut end = p.toks.len() - 1;
        for (j, t) in p
            .toks
            .iter()
            .enumerate()
            .take(p.toks.len() - 1)
            .skip(start_i)
        {
            if t.kind == "OP" && t.text == "(" {
                depth += 1;
            } else if t.kind == "OP" && (t.text == "," || t.text == ")") && depth == 0 {
                end = j;
                break;
            } else if t.kind == "OP" && t.text == ")" {
                depth -= 1;
            }
        }
        let last = if end > start_i {
            Some(&p.toks[end - 1])
        } else {
            None
        };
        if !need_op
            || (last.is_some_and(|l| {
                l.kind == "OP" && matches!(l.text.as_str(), "+" | "-" | "*" | "/" | "^")
            }) && p.toks[end..]
                .iter()
                .any(|t| t.kind == "OP" && t.text == "="))
        {
            let stop = if p.toks[end].kind == "EOF" {
                i64::try_from(pystr::len(&p.seg_text)).unwrap_or(0)
            } else {
                p.toks[end].col
            };
            let text = pystr::slice(&p.seg_text, Some(p.toks[start_i].col), Some(stop));
            p.i = p.toks.len() - 1;
            return Ok(Stmt::Unparsed(Unparsed {
                base: StmtBase::new(line, file),
                reason: format!("Const bad value: {kind}{text}"),
                expr: None,
            }));
        }
        Err(ParseError("Const subscript".to_string()))
    }

    /// Port of `parser.py::_Parser._const_array`
    pub fn _const_array(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        name_tok: &Token,
        dim: ExprRef,
        span: (i64, i64),
    ) -> Stmt {
        p.i = p.toks.len() - 1;
        if let Ok(val) = consteval::value_of(Some(&dim), &self.env) {
            let is_f = consteval::is_float(Some(&dim), &self.env);
            self.env.define(&name_tok.text, val, is_f);
        }
        Stmt::ConstDecl(ConstDecl {
            base: StmtBase::new(line, file),
            name: name_tok.text.clone(),
            value: Some(dim),
            type_: None,
            value_span: Some(span),
        })
    }

    /// Port of `parser.py::_Parser._parse_declaration`
    pub fn _parse_declaration(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        kind_in: &str,
    ) -> Result<Stmt, ParseError> {
        p.next();
        let mut kind = kind_in.to_string();
        if kind == "DIM" && p.is_ident("PUBLIC", 0) {
            p.next();
            kind = "PUBLIC".to_string();
        }
        if p.at_end() {
            return Ok(Stmt::Declaration(Declaration {
                base: StmtBase::new(line, file),
                kind,
                vars: Vec::new().into(),
            }));
        }
        let mut vars_: Vec<Rc<VarDecl>> = Vec::new();
        loop {
            if (kind == "PUBLIC" || kind == "DIM")
                && let Some(bad) = Self::_numeric_name(p)
            {
                self.diags.error_basic(
                    line,
                    &format!("Variable name {bad} illegal, starts with a number."),
                    file,
                );
            } else {
                vars_.push(self._parse_vardecl(p, line, file)?);
            }
            if !p.is_op(",", 0) {
                break;
            }
            p.next();
            if p.at_end() {
                break;
            }
        }
        if p.is_op(")", 0) || p.is_op("}", 0) {
            p.i = p.toks.len() - 1;
        }
        if vars_.is_empty() {
            let mut base = StmtBase::new(line, file);
            base.text.clone_from(&p.seg_text);
            return Ok(Stmt::Unparsed(Unparsed {
                base,
                reason: "empty statement".to_string(),
                expr: None,
            }));
        }
        Ok(Stmt::Declaration(Declaration {
            base: StmtBase::new(line, file),
            kind,
            vars: vars_.into(),
        }))
    }

    /// Port of `parser.py::_Parser._numeric_name`
    pub fn _numeric_name(p: &mut TokStream) -> Option<String> {
        if p.peek(0).kind != "NUMBER" {
            return None;
        }
        let target = if p.seg_text.is_empty() {
            p.peek(0).text.as_str()
        } else {
            pystr::slice(&p.seg_text, Some(p.peek(0).col), None)
        };
        let matched = _DIGIT_WORD_RE
            .match_(target)
            .map(|m| m.group0().to_string());
        let mut depth = 0i64;
        while !p.at_end() && !(depth == 0 && (p.is_op(",", 0) || p.is_op(")", 0))) {
            depth += i64::from(p.is_op("(", 0)) - i64::from(p.is_op(")", 0));
            p.next();
        }
        Some(matched.unwrap_or_else(|| p.peek(0).text.clone()))
    }

    /// Port of `parser.py::_Parser._close_dims_or_raise`
    pub fn _close_dims_or_raise(
        &self,
        p: &mut TokStream,
        name_tok: &Token,
    ) -> Result<(), ParseError> {
        if p.at_end() {
            let text = if p.seg_text.is_empty() {
                name_tok.text.as_str()
            } else {
                pystr::slice(&p.seg_text, Some(name_tok.col), None)
            };
            return Err(ParseError(format!("{UNCLOSED_DIM_PREFIX}{text}")));
        }
        p.expect_close(")")?;
        Ok(())
    }

    /// Port of `parser.py::_Parser._parse_dim`
    pub fn _parse_dim(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let expr = self._parse_expr(p, line, file)?;
        if !(matches!(p.peek(0).kind.as_str(), "NUMBER" | "STRING" | "IDENT") || p.is_op("(", 0)) {
            return Ok(expr);
        }
        let mut items: Vec<Option<ExprRef>> = vec![Some(expr)];
        while matches!(p.peek(0).kind.as_str(), "NUMBER" | "STRING" | "IDENT") || p.is_op("(", 0) {
            items.push(Some(self._parse_expr(p, line, file)?));
        }
        Ok(Rc::new(Expr::Juxtaposed(Juxtaposed {
            node: Node::new(line, file),
            items,
        })))
    }

    /// Port of `parser.py::_Parser._parse_vardecl`
    pub fn _parse_vardecl(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Rc<VarDecl>, ParseError> {
        let name_tok = p.expect_ident(None)?;
        let mut dims: Vec<ExprRef> = Vec::new();
        let mut dim0_span: Option<(i64, i64)> = None;
        let mut paren_dim = false;
        if !p.is_op("(", 0) && !p.is_ident("AS", 0) {
            let mut j = 0usize;
            while matches!(p.peek(j).kind.as_str(), "IDENT" | "NUMBER" | "STRING")
                || (j > 0 && p.peek(j).kind == "OP" && p.peek(j).text == ".")
            {
                j += 1;
            }
            if j > 0
                && p.peek(j).kind == "OP"
                && p.peek(j).text == "("
                && !(0..j).any(|k| p.peek(k).kind == "IDENT" && p.peek(k).upper == "AS")
            {
                p.i += j;
            }
        }
        if p.is_op("(", 0) {
            p.next();
            let first = p.i;
            if !p.is_op(")", 0) {
                let start_col = p.peek(0).col;
                dims.push(self._parse_dim(p, line, file)?);
                dim0_span = Some((start_col, p.peek(0).col));
                while p.is_op(",", 0) {
                    p.next();
                    dims.push(self._parse_dim(p, line, file)?);
                }
            }
            paren_dim = p.toks[first..p.i]
                .iter()
                .any(|t| t.kind == "OP" && t.text == "(");
            self._close_dims_or_raise(p, &name_tok)?;
            if paren_dim && dims.len() > 1 {
                let open_col = usize::try_from(p.toks[first - 1].col).unwrap_or(0);
                let close_col = usize::try_from(p.toks[p.i - 1].col).unwrap_or(0);
                if let Some(keep) = _dims_after_type_scan(&p.seg_text, open_col, close_col) {
                    dims.truncate(keep);
                }
            }
        }
        let mut vtype: Option<String> = None;
        let mut strsize: Option<ExprRef> = None;
        let mut empty_type_dims = false;
        let mut empty_type_init = false;
        let mut init: Option<Vec<Option<ExprRef>>> = None;
        let mut size_missing: Option<i64> = None;
        let mut size_junk: Option<(i64, String)> = None;
        if p.is_ident("AS", 0) {
            p.next();
            let type_tok = p.expect_ident(None)?;
            let mut vt = type_tok.upper;
            if p.is_op("!", 0) {
                p.next();
                vt.push('!');
            }
            if vt == "STRING" && p.is_op("*", 0) {
                let star = p.next();
                if p.at_end() || p.is_op(",", 0) {
                    size_missing = Some(star.col + 1);
                } else if dims.is_empty()
                    && p.peek(0).kind == "NUMBER"
                    && p.peek(1).kind == "OP"
                    && p.peek(1).text == "("
                {
                    let num = p.next();
                    let val = match num.value {
                        TokenValue::Int(n) => NumberValue::Int(n),
                        TokenValue::Float(f) => NumberValue::Float(f),
                        TokenValue::BigInt(s) => NumberValue::BigInt(s),
                        _ => NumberValue::Int(0),
                    };
                    strsize = Some(Rc::new(Expr::Number(Number {
                        node: Node::new(num.line, file),
                        value: val,
                        text: num.text,
                    })));
                    p.next();
                    let start_col = p.peek(0).col;
                    dims.push(self._parse_dim(p, line, file)?);
                    dim0_span = Some((start_col, p.peek(0).col));
                    while p.is_op(",", 0) {
                        p.next();
                        dims.push(self._parse_dim(p, line, file)?);
                    }
                    self._close_dims_or_raise(p, &name_tok)?;
                } else {
                    let size_col = p.peek(0).col;
                    strsize = Some(self._parse_expr(p, line, file)?);
                    if p.is_op("!", 0) || p.peek(0).kind == "IDENT" {
                        let kind = if p.is_op("!", 0) { "bang" } else { "word" };
                        size_junk = Some((size_col, kind.to_string()));
                    }
                }
            } else if dims.is_empty() && p.is_op("(", 0) {
                p.next();
                empty_type_dims = p.is_op(")", 0);
                if !p.is_op(")", 0) {
                    let start_col = p.peek(0).col;
                    dims.push(self._parse_dim(p, line, file)?);
                    dim0_span = Some((start_col, p.peek(0).col));
                    while p.is_op(",", 0) {
                        p.next();
                        dims.push(self._parse_dim(p, line, file)?);
                    }
                }
                self._close_dims_or_raise(p, &name_tok)?;
            }
            if p.is_op("=", 0) {
                empty_type_init = empty_type_dims;
                p.next();
                let val = self._parse_expr(p, line, file)?;
                init = Some(if let Some(items) = val.array_items() {
                    items.clone()
                } else {
                    vec![Some(val)]
                });
            }
            while !p.at_end() && !p.is_op(",", 0) {
                p.next();
            }
            vtype = Some(vt);
        } else if p.is_op("=", 0) {
            p.next();
            let val = self._parse_expr(p, line, file)?;
            init = Some(if let Some(items) = val.array_items() {
                items.clone()
            } else {
                vec![Some(val)]
            });
        } else if !(dims.is_empty() || p.at_end() || p.is_op(",", 0)) {
            let mut depth = 0i64;
            while !(p.at_end() || depth == 0 && p.is_op(",", 0)) {
                let tok = p.next();
                if tok.kind == "OP" && (tok.text == "(" || tok.text == "{") {
                    depth += 1;
                } else if tok.kind == "OP" && (tok.text == ")" || tok.text == "}") {
                    depth = (depth - 1).max(0);
                }
            }
        } else if !(p.at_end() || p.is_op(",", 0)) {
            let bad = p.peek(0);
            return Err(ParseError(format!("Invalid As clause: {}", bad.text)));
        }

        let mut junk_dims: Vec<ExprRef> = Vec::new();
        if dims.iter().any(|d| matches!(&**d, Expr::Juxtaposed(_))) {
            junk_dims = dims
                .iter()
                .map(|d| {
                    if matches!(&**d, Expr::Juxtaposed(_)) {
                        Rc::new(Expr::Number(Number {
                            node: Node::new(d.line(), file),
                            value: NumberValue::Int(1),
                            text: "1".to_string(),
                        }))
                    } else {
                        d.clone()
                    }
                })
                .collect();
            std::mem::swap(&mut junk_dims, &mut dims);
        }

        Ok(Rc::new(VarDecl {
            name: name_tok.text,
            dims,
            type_: vtype,
            string_size: strsize,
            init,
            units: None,
            dim0_span,
            paren_dim,
            junk_dims,
            empty_type_init,
            size_missing,
            size_junk,
        }))
    }

    /// Port of `parser.py::_Parser._parse_alias`
    pub fn _parse_alias(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next();
        let Ok(lhs) = self._parse_atom(p, line, file, true) else {
            return Ok(Stmt::Unparsed(Unparsed {
                base: StmtBase::new(line, file),
                reason: "Alias without '='".to_string(),
                expr: None,
            }));
        };
        if p.expect_op("=").is_err() {
            while !p.at_end() {
                p.next();
            }
            return Ok(Stmt::Unparsed(Unparsed {
                base: StmtBase::new(line, file),
                reason: "Alias target with no '='".to_string(),
                expr: None,
            }));
        }
        if p.at_end() {
            return Ok(Stmt::Unparsed(Unparsed {
                base: StmtBase::new(line, file),
                reason: "Alias nothing after '='".to_string(),
                expr: None,
            }));
        }
        if p.peek(0).kind == "IDENT" {
            let mut rhs: Vec<ExprRef> = Vec::new();
            let mut junk = false;
            let mut item_junk: Vec<bool> = Vec::new();
            while !p.at_end() && p.peek(0).kind == "IDENT" {
                rhs.push(self._parse_atom(p, line, file, true)?);
                item_junk.push(false);
                let mut depth = 0i64;
                while !(p.at_end() || depth == 0 && p.is_op(",", 0)) {
                    junk = junk || rhs.len() == 1;
                    *item_junk.last_mut().unwrap() = true;
                    let tok = p.next();
                    if tok.kind == "OP" && tok.text == "(" {
                        depth += 1;
                    } else if tok.kind == "OP" && tok.text == ")" {
                        depth = (depth - 1).max(0);
                    }
                }
                if p.at_end() {
                    break;
                }
                p.next(); // the comma
            }
            while !p.at_end() {
                p.next();
            }
            let mut args = vec![Some(lhs)];
            args.extend(rhs.into_iter().map(Some));
            return Ok(Stmt::Instruction(Instruction {
                base: StmtBase::new(line, file),
                keyword: "ALIAS".to_string(),
                args,
                alias_junk: junk,
                alias_item_junk: Some(item_junk),
                ..Default::default()
            }));
        }
        let mut rhs = vec![self._parse_expr(p, line, file)?];
        while p.is_op(",", 0) {
            p.next();
            rhs.push(self._parse_expr(p, line, file)?);
        }
        let mut args = vec![Some(lhs)];
        args.extend(rhs.into_iter().map(Some));
        Ok(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, file),
            keyword: "ALIAS".to_string(),
            args,
            ..Default::default()
        }))
    }

    /// Port of `parser.py::_Parser._parse_units_segment`
    pub fn _parse_units_segment(&mut self, seg_text: &str, line: i64, file: &str) -> Stmt {
        let Some(eq) = pystr::find(seg_text, "=", None, None) else {
            let stripped = pystr::strip(seg_text);
            let target_text = pystr::strip(pystr::slice(stripped, Some(5), None));
            if !target_text.is_empty()
                && let Ok(target_tokens) = tokenize_for_parser(target_text, line)
            {
                let mut tp = TokStream::new(target_tokens, "");
                if self._parse_atom(&mut tp, line, file, true).is_ok() && tp.at_end() {
                    let mut base = StmtBase::new(line, file);
                    base.text = target_text.to_string();
                    return Stmt::Unparsed(Unparsed {
                        base,
                        reason: "Units target with no '='".to_string(),
                        expr: None,
                    });
                }
            }
            let mut base = StmtBase::new(line, file);
            base.text = stripped.to_string();
            return Stmt::Unparsed(Unparsed {
                base,
                reason: "Units without '='".to_string(),
                expr: None,
            });
        };
        let lhs_text = pystr::slice(seg_text, None, Some(i64::try_from(eq).unwrap_or(0)));
        let label = pystr::strip(pystr::slice(
            seg_text,
            Some(i64::try_from(eq + 1).unwrap_or(0)),
            None,
        ));
        let lhs_tokens = match tokenize_for_parser(lhs_text, line) {
            Ok(t) => t,
            Err(e) => {
                let mut base = StmtBase::new(line, file);
                base.text = pystr::strip(seg_text).to_string();
                return Stmt::Unparsed(Unparsed {
                    base,
                    reason: format!("lex error in Units target: {}", e.message),
                    expr: None,
                });
            }
        };
        let mut p = TokStream::new(lhs_tokens, "");
        let parsed: Result<(ExprRef, Vec<ExprRef>), ParseError> = (|| {
            p.expect_ident(Some("UNITS"))?;
            let lhs = self._parse_atom(&mut p, line, file, true)?;
            let mut extra_targets: Vec<ExprRef> = Vec::new();
            if !p.at_end() && !matches!(&*lhs, Expr::Name(_) | Expr::Index(_)) {
                return Err(ParseError(
                    "unexpected trailing input in Units target".to_string(),
                ));
            }
            while p.is_op(",", 0) {
                p.next();
                if p.at_end() {
                    break;
                }
                let extra = self._parse_atom(&mut p, line, file, true)?;
                if !matches!(&*extra, Expr::Name(_) | Expr::Index(_)) {
                    return Err(ParseError(
                        "unexpected trailing input in Units target".to_string(),
                    ));
                }
                extra_targets.push(extra);
            }
            Ok((lhs, extra_targets))
        })();
        match parsed {
            Err(e) => {
                let mut base = StmtBase::new(line, file);
                base.text = pystr::strip(seg_text).to_string();
                Stmt::Unparsed(Unparsed {
                    base,
                    reason: e.0,
                    expr: None,
                })
            }
            Ok((lhs, extra_targets)) => {
                let mut base = StmtBase::new(line, file);
                base.text = pystr::strip(seg_text).to_string();
                Stmt::Instruction(Instruction {
                    base,
                    keyword: "UNITS".to_string(),
                    args: vec![
                        Some(lhs),
                        Some(Rc::new(Expr::String(StringLit {
                            node: Node::new(line, file),
                            value: label.to_string(),
                        }))),
                    ],
                    extra_targets,
                    ..Default::default()
                })
            }
        }
    }
}
