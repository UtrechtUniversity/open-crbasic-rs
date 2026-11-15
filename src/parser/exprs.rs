//! Port of `parser.py` lines 2902–3189: expression parsing (`_parse_expr` .. `_parse_bare_arglist`).

use std::rc::Rc;

use crate::pystr;
use crate::syntax::{
    ArrayLiteral, Binary, COMPOUND_ASSIGNMENT_OPS, Expr, ExprRef, Index, Member, Name, Node,
    Number, NumberValue, StringLit, TokenValue, Unary,
};

use super::Parser;
use super::helpers::{_RELOPS, _WORD_BINARY_OPS, ParseError, TokStream, UNCLOSED_PAREN_EXPR};

pub const _LOGICAL_OPS: [&str; 5] = ["IMP", "EQV", "OR", "XOR", "AND"];

fn number_value_from_token(val: &TokenValue) -> NumberValue {
    match val {
        TokenValue::Int(n) => NumberValue::Int(*n),
        TokenValue::Float(f) => NumberValue::Float(*f),
        _ => NumberValue::Int(0),
    }
}

impl Parser<'_> {
    /// Port of `parser.py::_Parser._parse_expr`
    pub fn _parse_expr(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        self._parse_logical(p, line, file)
    }

    /// Port of `parser.py::_Parser._parse_logical`
    pub fn _parse_logical(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_not(p, line, file)?;
        while p.peek(0).kind == "IDENT" && _LOGICAL_OPS.contains(&p.peek(0).upper.as_str()) {
            let op = p.next().upper;
            let right = if self.logical_assign && Self::_at_assign_target(p) {
                let target = self._parse_atom(p, line, file, true)?;
                p.next();
                let rhs = self._parse_logical(p, line, file)?;
                Rc::new(Expr::Binary(Binary {
                    node: Node::new(line, file),
                    op: "ASSIGN".to_string(),
                    left: Some(target),
                    right: Some(rhs),
                }))
            } else {
                self._parse_not(p, line, file)?
            };
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op,
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._at_assign_target`
    pub fn _at_assign_target(p: &TokStream) -> bool {
        if p.is_op("(", 0) {
            let mut depth = 0i64;
            let mut k = 0usize;
            while p.peek(k).kind != "EOF" {
                let t = p.peek(k);
                if t.kind == "OP" {
                    depth += match t.text.as_str() {
                        "(" => 1,
                        ")" => -1,
                        _ => 0,
                    };
                }
                k += 1;
                if depth == 0 {
                    break;
                }
            }
            return p.is_op("=", k);
        }
        let head = p.peek(0);
        if head.kind != "IDENT"
            || _WORD_BINARY_OPS.contains(&head.upper.as_str())
            || head.upper == "NOT"
        {
            return false;
        }
        let mut k = 1usize;
        while p.is_op(".", k) && p.peek(k + 1).kind == "IDENT" {
            k += 2;
        }
        if k > 1 && !p.is_op("(", k) {
            return p.is_op("=", k);
        }
        if p.is_op("(", k) {
            let mut depth = 0i64;
            while p.peek(k).kind != "EOF" {
                let t = p.peek(k);
                if t.kind == "OP" {
                    depth += match t.text.as_str() {
                        "(" => 1,
                        ")" => -1,
                        _ => 0,
                    };
                }
                k += 1;
                if depth == 0 {
                    break;
                }
            }
        }
        p.is_op("=", k)
    }

    /// Port of `parser.py::_Parser._parse_not`
    pub fn _parse_not(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        if p.is_ident("NOT", 0) {
            let tok = p.next();
            let operand = self._parse_not(p, line, file)?;
            return Ok(Rc::new(Expr::Unary(Unary {
                node: Node::new(tok.line, file),
                op: "NOT".to_string(),
                operand: Some(operand),
            })));
        }
        self._parse_relational(p, line, file)
    }

    /// Port of `parser.py::_Parser._parse_relational`
    pub fn _parse_relational(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_compound_operand(p, line, file)?;
        if self.to_ok && p.is_ident("TO", 0) {
            p.next();
            let mut nots = Vec::new();
            while p.is_ident("NOT", 0) {
                nots.push(p.next());
            }
            let mut right = self._parse_shift(p, line, file)?;
            for tok in nots.into_iter().rev() {
                right = Rc::new(Expr::Unary(Unary {
                    node: Node::new(tok.line, file),
                    op: "NOT".to_string(),
                    operand: Some(right),
                }));
            }
            return Ok(Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op: "TO".to_string(),
                left: Some(left),
                right: Some(right),
            })));
        }
        while p.peek(0).kind == "OP" && _RELOPS.contains(&p.peek(0).text.as_str()) {
            let op = p.next().text;
            let right = self._parse_compound_operand(p, line, file)?;
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op,
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_compound_operand`
    pub fn _parse_compound_operand(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let left = self._parse_shift(p, line, file)?;
        if p.peek(0).kind == "OP" && COMPOUND_ASSIGNMENT_OPS.contains(&p.peek(0).text.as_str()) {
            let op = p.next().text;
            let right = self._parse_compound_operand(p, line, file)?;
            return Ok(Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op,
                left: Some(left),
                right: Some(right),
            })));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_shift`
    pub fn _parse_shift(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_additive(p, line, file)?;
        while p.peek(0).kind == "OP" && matches!(p.peek(0).text.as_str(), "<<" | ">>") {
            let op = p.next().text;
            let right = self._parse_additive(p, line, file)?;
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op,
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_additive`
    pub fn _parse_additive(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_mod(p, line, file)?;
        while p.peek(0).kind == "OP" && matches!(p.peek(0).text.as_str(), "&" | "+" | "-") {
            let op = p.next().text;
            let right = self._parse_mod(p, line, file)?;
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op,
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_mod`
    pub fn _parse_mod(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_intdiv(p, line, file)?;
        while p.is_ident("MOD", 0) {
            p.next();
            let right = self._parse_intdiv(p, line, file)?;
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op: "MOD".to_string(),
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_intdiv`
    pub fn _parse_intdiv(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_muldiv(p, line, file)?;
        while (p.peek(0).kind == "OP" && p.peek(0).text == "\\") || p.is_ident("INTDV", 0) {
            p.next();
            let right = self._parse_muldiv(p, line, file)?;
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op: "\\".to_string(),
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_muldiv`
    pub fn _parse_muldiv(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_unary(p, line, file)?;
        while p.peek(0).kind == "OP" && matches!(p.peek(0).text.as_str(), "*" | "/") {
            let op = p.next().text;
            let right = self._parse_unary(p, line, file)?;
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op,
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_unary`
    pub fn _parse_unary(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        if p.peek(0).kind == "OP" && matches!(p.peek(0).text.as_str(), "-" | "+") {
            let tok = p.next();
            let operand = self._parse_unary(p, line, file)?;
            return Ok(Rc::new(Expr::Unary(Unary {
                node: Node::new(tok.line, file),
                op: tok.text,
                operand: Some(operand),
            })));
        }
        if p.is_ident("NOT", 0) {
            let tok = p.next();
            let operand = self._parse_unary(p, line, file)?;
            return Ok(Rc::new(Expr::Unary(Unary {
                node: Node::new(tok.line, file),
                op: "NOT".to_string(),
                operand: Some(operand),
            })));
        }
        self._parse_power(p, line, file)
    }

    /// Port of `parser.py::_Parser._parse_power`
    pub fn _parse_power(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        let mut left = self._parse_atom(p, line, file, true)?;
        while p.peek(0).kind == "OP" && p.peek(0).text == "^" {
            p.next();
            let right = self._parse_power_operand(p, line, file)?;
            left = Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op: "^".to_string(),
                left: Some(left),
                right: Some(right),
            }));
        }
        Ok(left)
    }

    /// Port of `parser.py::_Parser._parse_power_operand`
    pub fn _parse_power_operand(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<ExprRef, ParseError> {
        if p.peek(0).kind == "OP" && matches!(p.peek(0).text.as_str(), "-" | "+") {
            let tok = p.next();
            let operand = self._parse_power_operand(p, line, file)?;
            return Ok(Rc::new(Expr::Unary(Unary {
                node: Node::new(tok.line, file),
                op: tok.text,
                operand: Some(operand),
            })));
        }
        if p.is_ident("NOT", 0) {
            let tok = p.next();
            let operand = self._parse_power_operand(p, line, file)?;
            return Ok(Rc::new(Expr::Unary(Unary {
                node: Node::new(tok.line, file),
                op: "NOT".to_string(),
                operand: Some(operand),
            })));
        }
        self._parse_atom(p, line, file, true)
    }

    /// Port of `parser.py::_Parser._parse_atom`
    pub fn _parse_atom(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        strict: bool,
    ) -> Result<ExprRef, ParseError> {
        let tok = p.peek(0).clone();
        if tok.kind == "OP" && matches!(tok.text.as_str(), "!" | "@") {
            p.next();
            let operand = self._parse_atom(p, line, file, true)?;
            return Ok(Rc::new(Expr::Unary(Unary {
                node: Node::new(tok.line, file),
                op: tok.text,
                operand: Some(operand),
            })));
        }
        if tok.kind == "NUMBER" {
            p.next();
            return Ok(Rc::new(Expr::Number(Number {
                node: Node::new(tok.line, file),
                value: number_value_from_token(&tok.value),
                text: tok.text,
            })));
        }
        if tok.kind == "STRING" {
            p.next();
            let s = match tok.value {
                TokenValue::Str(s) => s,
                _ => String::new(),
            };
            return Ok(Rc::new(Expr::String(StringLit {
                node: Node::new(tok.line, file),
                value: s,
            })));
        }
        if tok.kind == "OP" && tok.text == "(" {
            p.next();
            let e = self._parse_expr(p, line, file)?;
            if strict && p.at_end() {
                return Err(ParseError(UNCLOSED_PAREN_EXPR.to_string()));
            }
            p.expect_close(")")?;
            return Ok(e);
        }
        if tok.kind == "OP" && tok.text == "{" {
            p.next();
            let mut items: Vec<Option<ExprRef>> = Vec::new();
            if !p.is_op("}", 0) {
                items.push(Some(self._parse_expr(p, line, file)?));
                while p.is_op(",", 0) {
                    p.next();
                    items.push(Some(self._parse_expr(p, line, file)?));
                }
            }
            p.expect_close("}")?;
            return Ok(Rc::new(Expr::ArrayLiteral(ArrayLiteral {
                node: Node::new(tok.line, file),
                items,
            })));
        }
        if tok.kind == "IDENT" {
            p.next();
            let mut node = Rc::new(Expr::Name(Name {
                node: Node::new(tok.line, file),
                name: tok.text.clone(),
            }));
            if p.is_op("(", 0) {
                p.next();
                let (args, _) = self._parse_arglist(p, ")", line, file, strict)?;
                node = Rc::new(Expr::Index(Index {
                    node: Node::new(tok.line, file),
                    name: tok.text.clone(),
                    args,
                }));
            }
            while p.is_op(".", 0) {
                if matches!(&*node, Expr::Name(_))
                    && (p.peek(1).kind != "IDENT" || p.peek(1).col > p.peek(0).col + 1)
                    && (self.profile.known_first_word_pass1 || p.i > 1)
                {
                    p.next();
                    node = Rc::new(Expr::Name(Name {
                        node: Node::new(tok.line, file),
                        name: format!("{}.", tok.text),
                    }));
                    break;
                }
                p.next();
                let member_tok = p.expect_ident(None)?;
                let mut margs = None;
                if p.is_op("(", 0) {
                    p.next();
                    let (a, _) = self._parse_arglist(p, ")", line, file, true)?;
                    margs = Some(a);
                }
                node = Rc::new(Expr::Member(Member {
                    node: Node::new(tok.line, file),
                    base: Some(node),
                    member: member_tok.text,
                    args: margs,
                }));
            }
            return Ok(node);
        }
        let found = if tok.text.is_empty() {
            &tok.kind
        } else {
            &tok.text
        };
        Err(ParseError(format!(
            "expected an expression, found {}",
            pystr::repr_str(found)
        )))
    }

    /// Port of `parser.py::_Parser._parse_arglist`
    pub fn _parse_arglist(
        &mut self,
        p: &mut TokStream,
        close: &str,
        line: i64,
        file: &str,
        strict: bool,
    ) -> Result<(Vec<Option<ExprRef>>, Vec<Option<(i64, i64)>>), ParseError> {
        let mut args: Vec<Option<ExprRef>> = Vec::new();
        let mut spans: Vec<Option<(i64, i64)>> = Vec::new();
        if p.is_op(close, 0) {
            p.next();
            return Ok((args, spans));
        }
        if !p.at_end() {
            loop {
                if p.at_end()
                    || (p.peek(0).kind == "OP"
                        && (p.peek(0).text == "," || p.peek(0).text == close))
                {
                    args.push(None);
                    spans.push(None);
                } else {
                    let start_col = p.peek(0).col;
                    args.push(Some(self._parse_expr(p, line, file)?));
                    spans.push(Some((start_col, p.peek(0).col)));
                }
                if p.is_op(",", 0) {
                    p.next();
                    continue;
                }
                break;
            }
        }
        if strict && p.at_end() {
            return Err(ParseError(UNCLOSED_PAREN_EXPR.to_string()));
        }
        p.expect_close(close)?;
        Ok((args, spans))
    }

    /// Port of `parser.py::_Parser._parse_bare_arglist`
    pub fn _parse_bare_arglist(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<(Vec<Option<ExprRef>>, Vec<Option<(i64, i64)>>), ParseError> {
        let start_col = p.peek(0).col;
        let mut args: Vec<Option<ExprRef>> = vec![Some(self._parse_expr(p, line, file)?)];
        let mut spans: Vec<Option<(i64, i64)>> = vec![Some((start_col, p.peek(0).col))];
        while p.is_op(",", 0) {
            p.next();
            let start_col = p.peek(0).col;
            args.push(Some(self._parse_expr(p, line, file)?));
            spans.push(Some((start_col, p.peek(0).col)));
        }
        Ok((args, spans))
    }
}
