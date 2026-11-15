//! Port of `parser.py` lines 1460–2282: statement parsing (`_parse_stmt` .. `_parse_param`).

use std::rc::Rc;
use std::sync::LazyLock;

use crate::pyre::Regex;
use crate::pystr;
use crate::syntax::{
    Assignment, Binary, COMPOUND_ASSIGNMENT_OPS, Call, Case, CaseItem, Declaration, DoLoop, ElseIf,
    Expr, ExprRef, For, If, Instruction, Name, Node, Param, Stmt, StmtBase, StmtRef, SubDef, Token,
    Unparsed, VarDecl,
};

use super::Parser;
use super::helpers::{
    _LOGICAL_LEAD_OPS, _RELOPS, _WORD_BINARY_OPS, _eof_like, _has_assign, _raw_word,
    _split_colon_segments, LOGICAL_ASSIGN, ParseError, TokStream,
};

static _WS_OR_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s()]", 0));

enum TailChunkResult {
    Expr(ExprRef, (i64, i64)),
    Empty,
    None,
}

fn is_assign_op(text: &str) -> bool {
    text == "=" || COMPOUND_ASSIGNMENT_OPS.contains(&text)
}

impl Parser {
    /// Port of `parser.py::_Parser._parse_stmt`
    pub fn _parse_stmt(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        _seg_text: &str,
    ) -> Result<Option<Stmt>, ParseError> {
        if p.at_end() {
            return Ok(None);
        }
        let first = p.peek(0).clone();
        if first.kind == "OP" && matches!(first.text.as_str(), "!" | "@") {
            return self._parse_assignment_or_call(p, line, file).map(Some);
        }
        if first.kind != "IDENT" {
            let start = p.i;
            if first.text == "(" && !self.profile.single_pass {
                if let Ok(stmt) = self._parse_assignment_or_call(p, line, file)
                    && let Stmt::Assignment(ref a) = stmt
                    && matches!(
                        a.target.as_deref(),
                        Some(Expr::Name(_) | Expr::Index(_) | Expr::Member(_))
                    )
                {
                    return Ok(Some(stmt));
                }
                p.i = start;
            }
            if first.kind == "OP"
                && first.text == "+"
                && let Some(result) = self._try_plus_assign(p, start, line, file)
            {
                return Ok(Some(result));
            }
            p.i = start;
            if matches!(first.kind.as_str(), "NUMBER" | "STRING")
                || (first.kind == "OP" && matches!(first.text.as_str(), "(" | "-" | "+"))
            {
                if let Some(result) = self._try_logical_assign(p, start, line, file) {
                    return Ok(Some(result));
                }
                p.i = start;
            }
            let found = if first.text.is_empty() {
                &first.kind
            } else {
                &first.text
            };
            return Err(ParseError(format!(
                "statement does not start with an identifier (found {})",
                pystr::repr_str(found)
            )));
        }
        let kw = first.upper.as_str();
        let stmt = match kw {
            "IF" => self._parse_if(p, line, file)?,
            "ELSEIF" => self._parse_elseif(p, line, file)?,
            "FOR" => self._parse_for(p, line, file)?,
            "DO" | "LOOP" => self._parse_doloop(p, line, file, kw)?,
            "SELECT" => self._parse_select(p, line, file)?,
            "CASE" => self._parse_case(p, line, file)?,
            "SUB" | "FUNCTION" => self._parse_subdef(p, line, file, kw)?,
            "CONST" => self._parse_const(p, line, file)?,
            "PUBLIC" | "DIM" if p.is_op(".", 1) => self._parse_assignment_or_call(p, line, file)?,
            "PUBLIC" | "DIM" => self._parse_declaration(p, line, file, kw)?,
            "ALIAS" => self._parse_alias(p, line, file)?,
            "UNITS" => {
                return Err(ParseError(
                    "Units nested inside another statement is not supported".to_string(),
                ));
            }
            "CALL" => self._parse_call(p, line, file)?,
            _ if self.profile.instructions().contains(kw) => {
                self._parse_generic_instruction(p, line, file, kw)?
            }
            _ => self._parse_assignment_or_call(p, line, file)?,
        };
        Ok(Some(stmt))
    }

    /// Port of `parser.py::_Parser._parse_generic_instruction`
    pub fn _parse_generic_instruction(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        kw: &str,
    ) -> Result<Stmt, ParseError> {
        let outer = self.to_ok;
        self.to_ok = true;
        let res = self._parse_generic_instruction_args(p, line, file, kw);
        self.to_ok = outer;
        res
    }

    /// Port of `parser.py::_Parser._parse_generic_instruction_args`
    pub fn _parse_generic_instruction_args(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        kw: &str,
    ) -> Result<Stmt, ParseError> {
        p.next();
        let mut args: Vec<Option<ExprRef>> = Vec::new();
        let mut spans: Vec<Option<(i64, i64)>> = Vec::new();
        if p.is_op("(", 0) {
            p.next();
            (args, spans) = self._parse_arglist(p, ")", line, file, false)?;
            self._absorb_trailing(p, &mut args, &mut spans, line, file, kw);
        } else if p.is_op("=", 0) {
            p.next();
            (args, spans) = self._parse_bare_arglist(p, line, file)?;
        } else if kw == "READONLY" {
            while !p.at_end() {
                if p.is_op(",", 0) {
                    p.next();
                    continue;
                }
                let start_col = p.peek(0).col;
                args.push(Some(self._parse_expr(p, line, file)?));
                spans.push(Some((start_col, p.peek(0).col)));
            }
        } else if !p.at_end() {
            (args, spans) = self._parse_bare_arglist(p, line, file)?;
            if p.is_op(";", 0) && kw == "EXIT" && !self.inline_body {
                self._absorb_trailing(p, &mut args, &mut spans, line, file, kw);
            }
        }
        if kw == "END"
            && args.len() == 1
            && matches!(
                args[0].as_deref(),
                Some(Expr::Name(n)) if pystr::upper(&n.name) == "TABLE"
            )
        {
            return Ok(Stmt::Instruction(Instruction {
                base: StmtBase::new(line, file),
                keyword: "ENDTABLE".to_string(),
                ..Default::default()
            }));
        }
        Ok(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, file),
            keyword: kw.to_string(),
            args,
            arg_spans: spans,
            ..Default::default()
        }))
    }

    /// Port of `parser.py::_Parser._absorb_trailing`
    pub fn _absorb_trailing(
        &mut self,
        p: &mut TokStream,
        args: &mut Vec<Option<ExprRef>>,
        spans: &mut Vec<Option<(i64, i64)>>,
        line: i64,
        file: &str,
        kw: &str,
    ) {
        let rest: Vec<Token> = p
            .remaining()
            .iter()
            .filter(|t| t.kind != "EOF")
            .cloned()
            .collect();
        if rest.is_empty() {
            return;
        }
        let mut pieces: Vec<Vec<Token>> = vec![Vec::new()];
        for t in &rest {
            if t.kind == "OP" && t.text == "," {
                pieces.push(Vec::new());
            } else {
                pieces.last_mut().unwrap().push(t.clone());
            }
        }
        let params = &self.profile.instructions().get(kw).unwrap().params;
        if args.len() >= params.len()
            && rest[0].kind == "OP"
            && rest[0].text == ";"
            && !self.inline_body
        {
            let mut end = 1usize;
            while end < rest.len()
                && rest[end].kind == "OP"
                && rest[end].text == ";"
                && rest[end].col == rest[end - 1].col + 1
            {
                end += 1;
            }
            let text = ";".repeat(end);
            args.push(Some(Rc::new(Expr::Name(Name {
                node: Node::new(line, file),
                name: text,
            }))));
            spans.push(Some((
                rest[0].col,
                rest[0].col + i64::try_from(end).unwrap_or(0),
            )));
            p.i = p.toks.len() - 1;
            return;
        }
        let mut found: Vec<(ExprRef, (i64, i64))> = Vec::new();
        let seg_text = p.seg_text.clone();
        for piece in &pieces {
            let slot = args.len() + found.len();
            let const_slot = slot >= params.len() || matches!(params[slot], 4 | 104);
            let got = self._tail_chunk(piece, &seg_text, line, file, const_slot);
            match got {
                TailChunkResult::Expr(e, sp) => found.push((e, sp)),
                TailChunkResult::Empty => {}
                TailChunkResult::None => {
                    if const_slot {
                        let mut groups: Vec<Vec<Token>> = if piece.is_empty() {
                            Vec::new()
                        } else {
                            vec![vec![piece[0].clone()]]
                        };
                        for w in piece.windows(2) {
                            let (a, b) = (&w[0], &w[1]);
                            if b.col == a.col + i64::try_from(pystr::len(&a.text)).unwrap_or(0) {
                                groups.last_mut().unwrap().push(b.clone());
                            } else {
                                groups.push(vec![b.clone()]);
                            }
                        }
                        if groups.is_empty() {
                            return;
                        }
                        let mut group_results: Vec<(ExprRef, (i64, i64))> = Vec::new();
                        for g in &groups {
                            match self._tail_chunk(g, &seg_text, line, file, true) {
                                TailChunkResult::Expr(e, sp) => group_results.push((e, sp)),
                                TailChunkResult::Empty => {}
                                TailChunkResult::None => return,
                            }
                        }
                        found.extend(group_results);
                    } else if !piece.is_empty() {
                        let Some((e, sp)) = self._whole_expr(piece, &seg_text, line, file) else {
                            return;
                        };
                        found.push((e, sp));
                    } else {
                        return;
                    }
                }
            }
        }
        for (expr, span) in found {
            args.push(Some(expr));
            spans.push(Some(span));
        }
        p.i = p.toks.len() - 1;
    }

    /// Port of `parser.py::_Parser._absorb_sub_tail`
    pub fn _absorb_sub_tail(
        &mut self,
        p: &mut TokStream,
        args: &mut Vec<Option<ExprRef>>,
        line: i64,
        file: &str,
    ) -> bool {
        let rest: Vec<Token> = p
            .remaining()
            .iter()
            .filter(|t| t.kind != "EOF")
            .cloned()
            .collect();
        if rest.is_empty() {
            return false;
        }
        if args.is_empty() {
            p.i = p.toks.len() - 1;
            return true;
        }
        let mut pieces: Vec<Vec<Token>> = vec![Vec::new()];
        let mut depth = 0i64;
        for t in &rest {
            if t.kind == "OP" && t.text == "(" {
                depth += 1;
            } else if t.kind == "OP" && t.text == ")" {
                depth = (depth - 1).max(0);
            }
            if t.kind == "OP" && t.text == "," && depth == 0 {
                pieces.push(Vec::new());
            } else {
                pieces.last_mut().unwrap().push(t.clone());
            }
        }
        if pieces.last().is_some_and(Vec::is_empty) {
            pieces.pop();
        }
        let mut found: Vec<Option<ExprRef>> = Vec::new();
        let seg_text = p.seg_text.clone();
        for piece in &pieces {
            if piece.is_empty() {
                found.push(None);
            } else if piece.len() == 1 && piece[0].text == ")" {
                if pieces.len() > 1 {
                    return false;
                }
                found.push(None);
            } else {
                let Some((expr, _)) = self._whole_expr(piece, &seg_text, line, file) else {
                    return false;
                };
                found.push(Some(expr));
            }
        }
        args.extend(found);
        p.i = p.toks.len() - 1;
        false
    }

    /// Port of `parser.py::_Parser._whole_expr`
    pub fn _whole_expr(
        &mut self,
        piece: &[Token],
        seg_text: &str,
        line: i64,
        file: &str,
    ) -> Option<(ExprRef, (i64, i64))> {
        let trimmed = if piece.last().is_some_and(|t| t.text == ")") {
            Some(&piece[..piece.len() - 1])
        } else {
            None
        };
        for cand in [Some(piece), trimmed].into_iter().flatten() {
            if cand.is_empty() {
                continue;
            }
            let mut toks = cand.to_vec();
            let last = cand.last().unwrap();
            toks.push(_eof_like(0));
            toks.last_mut().unwrap().col = last.col;
            let mut sub = TokStream::new(toks, seg_text);
            if let Ok(expr) = self._parse_expr(&mut sub, line, file)
                && sub.at_end()
            {
                let end_col = last.col + i64::try_from(pystr::len(&last.text)).unwrap_or(0);
                return Some((expr, (cand[0].col, end_col)));
            }
        }
        None
    }

    /// Port of `parser.py::_Parser._tail_chunk`
    fn _tail_chunk(
        &mut self,
        piece: &[Token],
        seg_text: &str,
        line: i64,
        file: &str,
        const_slot: bool,
    ) -> TailChunkResult {
        let mut lead = 0usize;
        while lead < piece.len() && piece[lead].kind == "OP" && piece[lead].text == "(" {
            lead += 1;
        }
        let filtered: Vec<Token> = piece
            .iter()
            .filter(|t| !(t.kind == "OP" && (t.text == ")" || t.text == "=")))
            .cloned()
            .collect();
        let loose: Vec<Token> = if lead > 0 && const_slot {
            piece[lead..]
                .iter()
                .filter(|t| !(t.kind == "OP" && (t.text == ")" || t.text == "=")))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        for (idx, chunk) in [piece, &filtered, &loose].into_iter().enumerate() {
            if chunk.is_empty() {
                if idx == 2 {
                    continue;
                }
                return TailChunkResult::Empty;
            }
            if chunk.windows(2).any(|w| {
                w[0].kind == "STRING"
                    || w[1].col != w[0].col + i64::try_from(pystr::len(&w[0].text)).unwrap_or(0)
            }) {
                continue;
            }
            let mut toks = chunk.to_vec();
            let last = chunk.last().unwrap();
            let mut eof = _eof_like(0);
            eof.col = last.col;
            toks.push(eof);
            let mut sub = TokStream::new(toks, seg_text);
            if let Ok(expr) = self._parse_expr(&mut sub, line, file)
                && sub.at_end()
            {
                let end_col = last.col + i64::try_from(pystr::len(&last.text)).unwrap_or(0);
                return TailChunkResult::Expr(expr, (chunk[0].col, end_col));
            }
        }
        TailChunkResult::None
    }

    /// Port of `parser.py::_Parser._parse_select`
    pub fn _parse_select(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next(); // SELECT
        p.expect_ident(Some("CASE"))?;
        let prev = self.to_ok;
        self.to_ok = true;
        let expr = self._parse_expr(p, line, file);
        self.to_ok = prev;
        let expr = expr?;
        Ok(Stmt::Instruction(Instruction {
            base: StmtBase::new(line, file),
            keyword: "SELECT".to_string(),
            args: vec![Some(expr)],
            ..Default::default()
        }))
    }

    /// Port of `parser.py::_Parser._parse_call`
    pub fn _parse_call(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next(); // CALL
        let mut name_tok = p.expect_ident(None)?;
        let mut j = p.i;
        while p.toks[j].kind == "IDENT" {
            j += 1;
        }
        if j > p.i && (p.toks[j].kind == "EOF" || (p.toks[j].kind == "OP" && p.toks[j].text == "("))
        {
            let mut merged = name_tok.text.clone();
            for t in &p.toks[p.i..j] {
                merged.push_str(&t.text);
            }
            name_tok.upper = pystr::upper(&merged);
            name_tok.text = merged;
            p.i = j;
        }
        let mut args: Vec<Option<ExprRef>> = Vec::new();
        if p.is_op("(", 0) {
            p.next();
            (args, _) = self._parse_arglist(p, ")", line, file, false)?;
            if args.len() > 1
                && args.last().is_some_and(Option::is_none)
                && !(p.toks[p.i - 1].kind == "OP" && p.toks[p.i - 1].text == ")")
            {
                args.pop();
            }
            let ignored = self._absorb_sub_tail(p, &mut args, line, file);
            return Ok(Stmt::Call(Call {
                base: StmtBase::new(line, file),
                name: name_tok.text,
                args,
                explicit: true,
                tail_ignored: ignored,
                ..Default::default()
            }));
        } else if p.is_op(";", 0) && !self.inline_body {
            args = vec![Some(Rc::new(Expr::Name(Name {
                node: Node::new(line, file),
                name: ";".to_string(),
            })))];
            p.i = p.toks.len() - 1;
        } else if !p.at_end() {
            (args, _) = self._parse_bare_arglist(p, line, file)?;
        }
        Ok(Stmt::Call(Call {
            base: StmtBase::new(line, file),
            name: name_tok.text,
            args,
            explicit: true,
            ..Default::default()
        }))
    }

    /// Port of `parser.py::_Parser._try_logical_assign`
    pub fn _try_logical_assign(
        &mut self,
        p: &mut TokStream,
        start: usize,
        line: i64,
        file: &str,
    ) -> Option<Stmt> {
        p.i = start;
        let saved = p.toks.clone();
        let mut n = saved.len() - 1;
        while (self.profile.name == "cr6" || self.profile.name == "cr1000x")
            && n > start
            && saved[n - 1].kind == "OP"
            && saved[n - 1].text == ";"
        {
            n -= 1;
        }
        let mut trimmed = saved[..n].to_vec();
        trimmed.push(saved.last().unwrap().clone());
        p.toks = trimmed;
        self.logical_assign = true;
        let expr = self._parse_expr(p, line, file).ok();
        self.logical_assign = false;
        if let Some(expr) = expr
            && p.at_end()
            && _has_assign(&expr)
        {
            return Some(Stmt::Unparsed(Unparsed {
                base: StmtBase::new(line, file),
                reason: LOGICAL_ASSIGN.to_string(),
                expr: Some(expr),
            }));
        }
        p.toks = saved;
        None
    }

    /// Port of `parser.py::_Parser._try_plus_assign`
    pub fn _try_plus_assign(
        &mut self,
        p: &mut TokStream,
        start: usize,
        line: i64,
        file: &str,
    ) -> Option<Stmt> {
        p.i = start;
        while p.is_op("+", 0) {
            p.next();
        }
        if !Self::_at_assign_target(p)
            || p.is_op("(", 0)
            || (self.profile.single_pass && !p.is_op("=", 1))
        {
            return None;
        }
        let target = self._parse_atom(p, line, file, true).ok()?;
        if !p.is_op("=", 0) {
            return None;
        }
        p.next();
        let value = self._parse_expr(p, line, file).ok()?;
        if !p.at_end() {
            return None;
        }
        Some(Stmt::Unparsed(Unparsed {
            base: StmtBase::new(line, file),
            reason: LOGICAL_ASSIGN.to_string(),
            expr: Some(Rc::new(Expr::Binary(Binary {
                node: Node::new(line, file),
                op: "ASSIGN".to_string(),
                left: Some(target),
                right: Some(value),
            }))),
        }))
    }

    /// Port of `parser.py::_Parser._parse_assignment_or_call`
    pub fn _parse_assignment_or_call(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        let start = p.i;
        if p.is_ident("NOT", 0)
            && let Some(result) = self._try_logical_assign(p, start, line, file)
        {
            return Ok(result);
        }
        p.i = start;
        let outer = self.to_ok;
        self.to_ok = true;
        let target_res = self._parse_atom(p, line, file, false);
        self.to_ok = outer;
        let target = match target_res {
            Ok(t) => t,
            Err(e) => {
                if let Some(result) = self._try_logical_assign(p, start, line, file) {
                    return Ok(result);
                }
                return Err(e);
            }
        };
        let t = p.peek(0).clone();
        if t.kind == "OP"
            && t.text == "="
            && let Expr::Index(ref idx) = *target
            && idx.args.is_empty()
            && p.toks[p.i + 1].kind == "EOF"
        {
            p.i = p.toks.len() - 1;
            return Ok(Stmt::Call(Call {
                base: StmtBase::new(line, file),
                name: idx.name.clone(),
                args: Vec::new(),
                explicit: false,
                tail_ignored: true,
                ..Default::default()
            }));
        }
        if t.kind == "OP" && is_assign_op(&t.text) {
            let op_tok = p.next();
            self.to_ok = true;
            let value_res = self._parse_expr(p, line, file);
            self.to_ok = false;
            let mut value = value_res?;
            if op_tok.text != "=" {
                let base_op = pystr::slice(&op_tok.text, Some(0), Some(1)).to_string();
                value = Rc::new(Expr::Binary(Binary {
                    node: Node::new(op_tok.line, file),
                    op: base_op,
                    left: Some(target.clone()),
                    right: Some(value),
                }));
            }
            return Ok(Stmt::Assignment(Assignment {
                base: StmtBase::new(line, file),
                target: Some(target),
                value: Some(value),
                compound: op_tok.text != "=",
            }));
        }
        if (p.peek(0).kind == "IDENT"
            && _WORD_BINARY_OPS.contains(&p.peek(0).upper.as_str())
            && p.peek(0).upper != "MOD")
            || (p.peek(0).kind == "OP" && _LOGICAL_LEAD_OPS.contains(&p.peek(0).text.as_str()))
        {
            let end = p.i;
            if let Some(result) = self._try_logical_assign(p, start, line, file) {
                return Ok(result);
            }
            p.i = end;
        }
        if self.in_struct
            && let Expr::Name(ref nm) = *target
            && p.is_ident("AS", 0)
        {
            p.next(); // AS
            let type_tok = p.expect_ident(None)?;
            let mut vtype = type_tok.upper;
            if p.is_op("!", 0) {
                p.next();
                vtype.push('!');
            }
            let mut strsize = None;
            if vtype == "STRING" && p.is_op("*", 0) {
                p.next();
                strsize = Some(self._parse_expr(p, line, file)?);
            }
            let var = Rc::new(VarDecl {
                name: nm.name.clone(),
                type_: Some(vtype),
                string_size: strsize,
                ..Default::default()
            });
            return Ok(Stmt::Declaration(Declaration {
                base: StmtBase::new(line, file),
                kind: "STRUCTUREMEMBER".to_string(),
                vars: vec![var],
            }));
        }
        if let Expr::Name(ref nm) = *target {
            return Ok(Stmt::Call(Call {
                base: StmtBase::new(line, file),
                name: nm.name.clone(),
                args: Vec::new(),
                explicit: false,
                ..Default::default()
            }));
        }
        if let Expr::Index(ref idx) = *target {
            let up = pystr::upper(&idx.name);
            let is_fn = self.function_names.contains(&up)
                || (self.profile.single_pass
                    && self.profile.functions().contains(&up)
                    && !self.profile.instructions().contains(&up));
            if is_fn {
                if p.at_end() {
                    return Ok(Stmt::Call(Call {
                        base: StmtBase::new(line, file),
                        name: idx.name.clone(),
                        args: idx.args.clone(),
                        explicit: false,
                        ..Default::default()
                    }));
                }
                let t = p.peek(0).clone();
                if (matches!(t.kind.as_str(), "IDENT" | "NUMBER" | "STRING")
                    && !_WORD_BINARY_OPS.contains(&t.upper.as_str()))
                    || (t.kind == "OP" && matches!(t.text.as_str(), "(" | ","))
                {
                    let mut juxt: Vec<ExprRef> = Vec::new();
                    let comma = t.kind == "OP";
                    while !p.at_end() {
                        if p.peek(0).kind == "OP" && p.peek(0).text == "," {
                            p.i += 1;
                            continue;
                        }
                        juxt.push(self._parse_expr(p, line, file)?);
                    }
                    return Ok(Stmt::Call(Call {
                        base: StmtBase::new(line, file),
                        name: idx.name.clone(),
                        args: idx.args.clone(),
                        explicit: false,
                        juxtaposed: Some(juxt),
                        juxt_opaque: comma,
                        ..Default::default()
                    }));
                }
                p.i = start;
                let expr = self._parse_expr(p, line, file)?;
                if !p.at_end() {
                    let t = p.peek(0);
                    if !(self.profile.single_pass
                        && matches!(t.kind.as_str(), "IDENT" | "NUMBER" | "STRING")
                        && !_WORD_BINARY_OPS.contains(&t.upper.as_str()))
                    {
                        return Err(ParseError("expected an assignment or a call".to_string()));
                    }
                    let mut juxt: Vec<ExprRef> = Vec::new();
                    while !p.at_end() {
                        juxt.push(self._parse_expr(p, line, file)?);
                    }
                    return Ok(Stmt::Call(Call {
                        base: StmtBase::new(line, file),
                        name: idx.name.clone(),
                        args: idx.args.clone(),
                        explicit: false,
                        tail_expr: Some(expr),
                        juxtaposed: Some(juxt),
                        ..Default::default()
                    }));
                }
                return Ok(Stmt::Call(Call {
                    base: StmtBase::new(line, file),
                    name: idx.name.clone(),
                    args: idx.args.clone(),
                    explicit: false,
                    tail_expr: Some(expr),
                    ..Default::default()
                }));
            }
            let mut args = idx.args.clone();
            if args.len() > 1
                && args.last().is_some_and(Option::is_none)
                && !(p.toks[p.i - 1].kind == "OP" && p.toks[p.i - 1].text == ")")
            {
                args.pop();
            }
            let ignored = self._absorb_sub_tail(p, &mut args, line, file);
            return Ok(Stmt::Call(Call {
                base: StmtBase::new(line, file),
                name: idx.name.clone(),
                args,
                explicit: false,
                tail_ignored: ignored,
                ..Default::default()
            }));
        }
        Err(ParseError("expected an assignment or a call".to_string()))
    }

    /// Port of `parser.py::_Parser._parse_if`
    pub fn _parse_if(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next(); // IF
        let outer = self.to_ok;
        self.to_ok = true;
        let cond_res = self._parse_expr(p, line, file);
        self.to_ok = outer;
        let cond = cond_res?;
        let mut then_tok: Option<Token> = None;
        if p.is_ident("THEN", 0) {
            then_tok = Some(p.next());
        } else if !p.at_end() {
            let pk = p.peek(0);
            let found = if pk.text.is_empty() {
                &pk.kind
            } else {
                &pk.text
            };
            return Err(ParseError(format!(
                "expected THEN, found {}",
                pystr::repr_str(found)
            )));
        }
        if p.at_end() {
            return Ok(Stmt::If(If {
                base: StmtBase::new(line, file),
                cond: Some(cond),
                inline: None,
                inline_else: None,
            }));
        }
        let rest: Vec<Token> = p
            .remaining()
            .iter()
            .filter(|t| t.kind != "EOF")
            .cloned()
            .collect();
        p.i = p.toks.len() - 1;
        let mut depth = 0i64;
        let mut else_idx: Option<usize> = None;
        for (idx, t) in rest.iter().enumerate() {
            if t.kind == "OP" && t.text == "(" {
                depth += 1;
            } else if t.kind == "OP" && t.text == ")" {
                depth = (depth - 1).max(0);
            } else if t.kind == "IDENT" && t.upper == "ELSE" && depth == 0 {
                else_idx = Some(idx);
                break;
            }
        }
        let (then_toks, else_toks) = match else_idx {
            None => (rest.clone(), None),
            Some(ei) => (rest[..ei].to_vec(), Some(rest[ei + 1..].to_vec())),
        };
        let text_end = i64::try_from(pystr::len(&p.seg_text)).unwrap_or(0);
        let else_tok = else_idx.map(|ei| rest[ei].clone());
        let then_bounds = (
            then_tok
                .as_ref()
                .map(|tt| tt.col + i64::try_from(pystr::len(&tt.text)).unwrap_or(0) + 1),
            else_tok.as_ref().map_or(text_end, |et| et.col - 1),
        );
        let seg_text = p.seg_text.clone();
        let then_stmts = self._parse_stmt_list(then_toks, line, file, &seg_text, Some(then_bounds));
        let inline_else = else_toks.map(|etoks| {
            let et = else_tok.as_ref().unwrap();
            let start = et.col + i64::try_from(pystr::len(&et.text)).unwrap_or(0) + 1;
            self._parse_stmt_list(etoks, line, file, &seg_text, Some((Some(start), text_end)))
        });
        Ok(Stmt::If(If {
            base: StmtBase::new(line, file),
            cond: Some(cond),
            inline: Some(then_stmts),
            inline_else,
        }))
    }

    /// Port of `parser.py::_Parser._parse_stmt_list`
    pub fn _parse_stmt_list(
        &mut self,
        mut toks: Vec<Token>,
        line: i64,
        file: &str,
        outer: &str,
        bounds: Option<(Option<i64>, i64)>,
    ) -> Vec<StmtRef> {
        toks.push(_eof_like(line));
        let segs = _split_colon_segments(&toks);
        let outer_len = i64::try_from(pystr::len(outer)).unwrap_or(0);
        let mut out: Vec<StmtRef> = Vec::new();
        for (seg_idx, seg) in segs.into_iter().enumerate() {
            let real: Vec<Token> = seg.iter().filter(|t| t.kind != "EOF").cloned().collect();
            if real.is_empty() {
                continue;
            }
            let mut start = real[0].col;
            let last = real.last().unwrap();
            let mut end = last.col + i64::try_from(pystr::len(&last.text)).unwrap_or(0);
            if let Some(b) = bounds
                && seg_idx == 0
            {
                if let Some(b0) = b.0 {
                    start = start.min(b0);
                }
                end = end.max(b.1);
            }
            let verbatim = if !outer.is_empty() && outer_len >= end {
                pystr::slice(outer, Some(start), Some(end)).to_string()
            } else {
                String::new()
            };
            let fallback_text = if verbatim.is_empty() {
                real.iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                verbatim.clone()
            };
            self.inline_body = true;
            let mut stmt = self._parse_segment(seg, line, file, &fallback_text);
            self.inline_body = false;
            if !verbatim.is_empty() {
                stmt.base_mut().text = verbatim;
                if let Stmt::Instruction(ref mut instr) = stmt
                    && !instr.arg_spans.is_empty()
                {
                    instr.arg_spans = instr
                        .arg_spans
                        .iter()
                        .map(|sp| {
                            sp.map(|(s, e)| (s - start, (if e >= s { e } else { end }) - start))
                        })
                        .collect();
                }
            }
            stmt.base_mut().at_eol.set(false);
            out.push(Rc::new(stmt));
        }
        out
    }

    /// Port of `parser.py::_Parser._parse_elseif`
    pub fn _parse_elseif(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next(); // ELSEIF
        let outer = self.to_ok;
        self.to_ok = true;
        let cond_res = self._parse_expr(p, line, file);
        self.to_ok = outer;
        let cond = cond_res?;
        if p.is_ident("THEN", 0) {
            p.next();
            p.i = p.toks.len() - 1;
        }
        Ok(Stmt::ElseIf(ElseIf {
            base: StmtBase::new(line, file),
            cond: Some(cond),
        }))
    }

    /// Port of `parser.py::_Parser._parse_for`
    pub fn _parse_for(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next(); // FOR
        let var = match self._parse_atom(p, line, file, true) {
            Ok(v) => {
                if p.expect_op("=").is_err() {
                    return Err(ParseError("For missing parameter".to_string()));
                }
                v
            }
            Err(_) => return Err(ParseError("For missing parameter".to_string())),
        };
        if p.at_end() {
            return Err(ParseError("For missing parameter".to_string()));
        }
        let start = self._parse_expr(p, line, file)?;
        if p.at_end() {
            return Err(ParseError("For missing parameter".to_string()));
        }
        p.expect_ident(Some("TO"))?;
        if p.at_end() {
            return Err(ParseError("For missing parameter".to_string()));
        }
        let end = self._parse_expr(p, line, file)?;
        let mut step = None;
        if p.is_ident("STEP", 0) {
            p.next();
            step = Some(self._parse_expr(p, line, file)?);
        }
        Ok(Stmt::For(For {
            base: StmtBase::new(line, file),
            var: Some(var),
            start: Some(start),
            end: Some(end),
            step,
        }))
    }

    /// Port of `parser.py::_Parser._parse_doloop`
    pub fn _parse_doloop(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        which: &str,
    ) -> Result<Stmt, ParseError> {
        p.next(); // DO or LOOP
        let mut cond_kind = None;
        let mut cond = None;
        if p.is_ident("WHILE", 0) || p.is_ident("UNTIL", 0) {
            let tok = p.next();
            cond_kind = Some(tok.upper);
            cond = Some(self._parse_expr(p, line, file)?);
        } else if !p.at_end() {
            let t = if p.is_op("(", 0) && p.peek(1).kind == "IDENT" {
                p.peek(1)
            } else {
                p.peek(0)
            };
            if t.kind == "IDENT" || t.kind == "STRING" {
                return Err(ParseError(format!(
                    "Do/Loop parameter: Illegal parameter {}, expecting constant integer.",
                    t.text
                )));
            }
            if (t.kind == "NUMBER" && p.peek(1).kind == "EOF")
                || (t.kind == "OP"
                    && t.text == "-"
                    && p.peek(1).kind == "NUMBER"
                    && p.peek(2).kind == "EOF")
            {
                return Err(ParseError(
                    "Do/Loop parameter: Invalid or missing expression.".to_string(),
                ));
            }
        }
        Ok(Stmt::DoLoop(DoLoop {
            base: StmtBase::new(line, file),
            keyword: which.to_string(),
            cond_kind,
            cond,
        }))
    }

    /// Port of `parser.py::_Parser._parse_case`
    pub fn _parse_case(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
    ) -> Result<Stmt, ParseError> {
        p.next(); // CASE
        if p.is_ident("ELSE", 0) && p.peek(1).kind == "EOF" {
            p.next();
            return Ok(Stmt::Case(Case {
                base: StmtBase::new(line, file),
                items: Vec::new(),
                is_else: true,
            }));
        }
        let mut items: Vec<CaseItem> = Vec::new();
        loop {
            if p.is_ident("IS", 0) {
                p.next();
                let op_tok = p.next();
                if !(op_tok.kind == "OP" && _RELOPS.contains(&op_tok.text.as_str())) {
                    return Err(ParseError(format!(
                        "expected a comparison after Is, found {}",
                        pystr::repr_str(&op_tok.text)
                    )));
                }
                let e = self._parse_expr(p, line, file)?;
                items.push(CaseItem::Is(op_tok.text, e));
            } else {
                let lo = self._parse_expr(p, line, file)?;
                if p.is_ident("TO", 0) {
                    p.next();
                    let hi = self._parse_expr(p, line, file)?;
                    items.push(CaseItem::Range(lo, hi));
                } else {
                    items.push(CaseItem::Value(lo));
                }
            }
            if p.is_op(",", 0) {
                p.next();
                continue;
            }
            break;
        }
        Ok(Stmt::Case(Case {
            base: StmtBase::new(line, file),
            items,
            is_else: false,
        }))
    }

    /// Port of `parser.py::_Parser._parse_subdef`
    pub fn _parse_subdef(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        which: &str,
    ) -> Result<Stmt, ParseError> {
        p.next();
        let name_tok = p.expect_ident(None)?;
        let mut params: Vec<Rc<Param>> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        let mut after_optional = false;
        let mut optional_error = false;
        if p.is_op("(", 0) {
            p.next();
            if !p.is_op(")", 0) && !p.at_end() {
                loop {
                    if p.is_op(",", 0) {
                        p.next();
                        if p.at_end() || p.is_op(")", 0) {
                            break;
                        }
                        continue;
                    }
                    if let Some(bad) = Self::_numeric_name(p) {
                        errors.push(format!(
                            "Variable name {bad} illegal, starts with a number."
                        ));
                    } else {
                        let optional = p.is_ident("OPTIONAL", 0);
                        let check_optional = !self.profile.single_pass || !optional_error;
                        let missing_marker = check_optional && after_optional && !optional;
                        if missing_marker {
                            let word = if self.profile.single_pass {
                                "paramter"
                            } else {
                                "parameter"
                            };
                            errors.push(format!(
                                "All arguments after the first optional {word} must be optional."
                            ));
                        }
                        let before = errors.len();
                        let param =
                            self._parse_param(p, line, file, Some(&mut errors), after_optional)?;
                        let missing_default = check_optional
                            && (optional || after_optional)
                            && param.default.is_none();
                        if missing_default {
                            errors.push(format!(
                                "Optional parameter {} requires initialization.",
                                param.name
                            ));
                        }
                        params.push(param);
                        optional_error |= missing_marker
                            || missing_default
                            || errors[before..]
                                .iter()
                                .any(|err| err.starts_with("Variable Type "));
                        after_optional |= optional;
                    }
                    if !p.is_op(",", 0) {
                        break;
                    }
                    p.next();
                    if p.at_end() || p.is_op(")", 0) {
                        break;
                    }
                }
            }
            p.expect_close(")")?;
        }
        let mut return_type: Option<String> = None;
        if which == "FUNCTION" && p.is_ident("AS", 0) {
            p.next();
            let type_tok = p.expect_ident(None)?;
            let mut rtype = type_tok.upper;
            if p.is_op("!", 0) {
                p.next();
                rtype.push('!');
            }
            if rtype == "STRING" && p.is_op("*", 0) {
                p.next();
                self._parse_expr(p, line, file)?;
            }
            return_type = Some(rtype);
            if !p.at_end() {
                errors.push(format!(
                    "Too many parameters:  {}.",
                    _raw_word(p, " ,:()=>")
                ));
                p.i = p.toks.len() - 1;
            }
        } else if !params.is_empty() && !p.at_end() && p.peek(0).kind == "IDENT" {
            params.push(self._parse_param(p, line, file, Some(&mut errors), false)?);
            p.i = p.toks.len() - 1;
        }
        Ok(Stmt::SubDef(SubDef {
            base: StmtBase::new(line, file),
            keyword: which.to_string(),
            name: name_tok.text,
            params,
            return_type,
            header_errors: errors,
        }))
    }

    /// Port of `parser.py::_Parser._parse_param`
    pub fn _parse_param(
        &mut self,
        p: &mut TokStream,
        line: i64,
        file: &str,
        mut errors: Option<&mut Vec<String>>,
        after_optional: bool,
    ) -> Result<Rc<Param>, ParseError> {
        let optional = p.is_ident("OPTIONAL", 0);
        if optional {
            p.next();
        }
        let name_tok =
            if p.peek(0).kind == "STRING" && _WS_OR_PAREN_RE.search(&p.peek(0).text).is_none() {
                p.next()
            } else {
                p.expect_ident(None)?
            };
        let mut dims = 0i64;
        let mut dim_exprs: Vec<ExprRef> = Vec::new();
        if p.is_op("(", 0) {
            p.next();
            if !p.is_op(")", 0) {
                dim_exprs.push(self._parse_dim(p, line, file)?);
                while p.is_op(",", 0) {
                    p.next();
                    dim_exprs.push(self._parse_dim(p, line, file)?);
                }
            }
            p.expect_close(")")?;
            dims = 1;
        }
        if let Some(errs) = errors.as_deref_mut()
            && p.peek(0).kind == "IDENT"
            && !p.is_ident("AS", 0)
        {
            errs.push(format!(
                "Invalid Variable declaration {}.",
                _raw_word(p, " ,)")
            ));
            p.next();
        }
        let mut vtype: Option<String> = None;
        let mut strsize: Option<ExprRef> = None;
        let mut default: Option<ExprRef> = None;
        if p.is_ident("AS", 0) {
            p.next();
            let first = p.peek(0).clone();
            let second = p.peek(1).clone();
            if let Some(errs) = errors.as_deref_mut()
                && first.kind == "IDENT"
                && second.kind == "OP"
                && second.text == "="
                && second.col == first.col + i64::try_from(pystr::len(&first.text)).unwrap_or(0)
            {
                errs.push(format!(
                    "Variable Type {} not supported",
                    _raw_word(p, " ,)")
                ));
            }
            let type_tok = if p.at_end() {
                None
            } else {
                Some(p.expect_ident(None)?)
            };
            let mut vt = type_tok.map(|t| t.upper);
            if p.is_op("!", 0) {
                p.next();
                if let Some(ref mut s) = vt {
                    s.push('!');
                }
            }
            if vt.as_deref() == Some("STRING") && p.is_op("*", 0) {
                p.next();
                let size_col = p.peek(0).col;
                let parsed_size = self._parse_expr(p, line, file)?;
                if (optional || after_optional)
                    && let Expr::Binary(ref b) = *parsed_size
                    && b.op == "="
                {
                    strsize.clone_from(&b.left);
                    default.clone_from(&b.right);
                } else {
                    strsize = Some(parsed_size);
                }
                if p.is_op("!", 0)
                    && !self.profile.single_pass
                    && let Some(errs) = errors.as_deref_mut()
                {
                    while !p.at_end() && !p.is_op(",", 0) && !p.is_op(")", 0) {
                        p.next();
                    }
                    let echo = pystr::rstrip(pystr::slice(
                        &p.seg_text,
                        Some(size_col),
                        Some(p.peek(0).col),
                    ));
                    errs.push(format!("Incomplete expression: {echo}."));
                }
            }
            vtype = vt;
        }
        if p.is_op("=", 0) {
            if name_tok.kind == "STRING" && !optional {
                return Err(ParseError(
                    "required quoted parameter cannot be initialized".to_string(),
                ));
            }
            p.next();
            default = Some(self._parse_expr(p, line, file)?);
            if let Some(errs) = errors
                && !(optional || after_optional)
                && name_tok.kind == "IDENT"
            {
                let name = if self.profile.single_pass {
                    "p"
                } else {
                    &name_tok.text
                };
                errs.push(format!("Required parameter {name} cannot be initialized."));
            }
        }
        Ok(Rc::new(Param {
            name: name_tok.text,
            by_ref: None,
            type_: vtype,
            string_size: strsize,
            dims,
            default,
            dim_exprs,
        }))
    }
}
