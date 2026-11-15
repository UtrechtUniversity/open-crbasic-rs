//! Port of `parser.py` lines 696–1459: `_Parser` driver, directives, line/segment parsing.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::LazyLock;

use crate::consteval;
use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::pyre::{self, Regex};
use crate::pystr;
use crate::syntax::{
    ConstDecl, ConstValue, Declaration, Expr, ExprRef, Instruction, LineSpan, LogicalLine, Node,
    Number, NumberValue, Program, Stmt, StmtBase, StmtRef, Token, TokenValue, Unparsed,
    hash_statement,
};

use super::Parser;
use super::helpers::{
    _DEFERRED_SOURCE_INSTRUCTIONS, _DIGIT_NAME, _DIRECTIVE_RE, _FUNCTION_DEF_RE, _LEADING_WORD_RE,
    _RAW_NAME_FIRST, _SNMP_OID_STATEMENT, _hoist_block_statements, _split_text_segments, CondFrame,
    ParseError, SNMP_OID_REASON, TokStream,
};
use super::{_cr3xx_include_exts, _include_own_ext, tokenize_for_parser};

const _ABORTING_PARAMS: [i64; 12] = [2, 8, 24, 25, 26, 27, 28, 29, 33, 40, 41, 47];

static _THEN_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bThen\b", pyre::I));
static _COLON_IF_END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*(?:Else|EndIf|End\s+If)\s*", pyre::I));
static _COLON_IF_THEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bThen\s*$", pyre::I));
static _BAD_COND_INLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*If\b.*?\bThen\b(\s*\S.*)$", pyre::I | pyre::S));
static _SUB_UNTERM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*(?:Sub|Function)\s+[A-Za-z_]\w*\s*\(\s*", pyre::I));
static _ESS_INIT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*ESSInitialize\b", pyre::I));
static _LEAD_IDENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*([A-Za-z_]\w*)", 0));
static _SLOW_SHAPE1_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*SlowSequence\s*\(\s*", pyre::I));
static _SLOW_SHAPE2_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*SlowSequence\s*\(\s*[^(),]+,\s*", pyre::I));
static _CONST_BRACKET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(\s*Const\s+([A-Za-z_]\w*)\s+As\s+)([^\s,]*\[[^\s,]*)",
        pyre::I,
    )
});
static _CONST_QUOTE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\s*Const\s+""#, pyre::I));
static _CONST_NON_SPACE_COMMA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\s,]+", 0));
static _CONST_NUM_BEFORE_QUOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s*Const\s+[^\s",]+\s+"#, pyre::I));
static _CONST_IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*Const\s+[A-Za-z_]", pyre::I));
static _CR3XX_SEMI_NEXT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*([^\s,()]+)", 0));
static _SNMP_ERR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"expected '\)', found '\.\d+'", 0));

impl Parser {
    /// Port of `parser.py::_Parser.run`
    pub fn run(&mut self, lines: &[LogicalLine], name: &str) -> (Program, Diagnostics) {
        self.program.name = name.to_string();
        if !self.collect
            && lines
                .iter()
                .any(|ll| _FUNCTION_DEF_RE.match_(&ll.text).is_some())
        {
            let mut probe = Parser::new(self.profile, Diagnostics::new(), self.provenance);
            probe.collect = true;
            probe.run(lines, name);
            self.function_names = probe.function_names;
        }
        let mut skipped_include_depth: usize = 0;
        let mut includes_stopped = false;
        let mut todo: VecDeque<LogicalLine> = lines.iter().cloned().collect();
        let mut unshift: HashMap<String, i64> = HashMap::new();
        while let Some(mut ll) = todo.pop_front() {
            if let Some(&shift) = unshift.get(&ll.file)
                && shift != 0
            {
                ll.line += shift;
            }
            if let Some(ref unpulled) = ll.unpulled
                && skipped_include_depth == 0
                && !self._active()
            {
                for u in unpulled.iter().rev() {
                    todo.push_front(u.clone());
                }
                *unshift.entry(ll.file.clone()).or_insert(0) += ll.pulls;
                continue;
            }
            self._note_line(&ll);
            if skipped_include_depth > 0 {
                if ll.kind == "include_begin" {
                    skipped_include_depth += 1;
                } else if ll.kind == "include_end" {
                    skipped_include_depth -= 1;
                }
                continue;
            }
            if ll.kind == "include_begin" {
                if !self._active() || includes_stopped {
                    skipped_include_depth = 1;
                }
                continue;
            }
            if ll.kind == "include_end" {
                continue;
            }
            if ll.kind == "include_warning" || ll.kind == "include_error" {
                if self._active() {
                    if ll.kind == "include_warning" {
                        self.diags.warning_basic(ll.line, &ll.text, &ll.file);
                    } else {
                        self.diags.error_basic(ll.line, &ll.text, &ll.file);
                        self.program
                            .include_error_at
                            .push(self.program.statements.len());
                    }
                    includes_stopped |= ll.stops_includes && !self.profile.single_pass;
                }
                continue;
            }
            if ll.kind == "overflow" {
                self.diags
                    .error_basic(ll.line, "Line exceeds max characters of 512.", &ll.file);
                break;
            }
            let dir_match = _DIRECTIVE_RE.match_(&ll.text).and_then(|m| {
                let kind = pystr::upper(m.group(1).unwrap_or(""));
                if matches!(kind.as_str(), "ELSEIF" | "ELSE" | "ENDIF")
                    && self.cond_stack.is_empty()
                {
                    None
                } else {
                    Some((kind, m.group(2).unwrap_or("").to_string()))
                }
            });
            if let Some((mut kind, rest)) = dir_match {
                if kind == "IF_NO_REMOVE" {
                    kind = "IF".to_string();
                }
                self.env.section.clone_from(&ll.section);
                self._handle_directive(&kind, &rest, ll.line, &ll.file);
                continue;
            }
            if !self._active() {
                continue;
            }
            self.env.section.clone_from(&ll.section);
            if self.collect
                && let Some(m) = _FUNCTION_DEF_RE.match_(&ll.text)
            {
                self.function_names
                    .insert(pystr::upper(m.group(1).unwrap_or("")));
            }
            if let Some((ref msg, ref var)) = ll.bad_init {
                self.diags.error_basic(ll.line, msg, &ll.file);
                self.diags.error_basic(
                    ll.line,
                    &format!("Variable initialization of {var} has bad syntax."),
                    &ll.file,
                );
            }
            let before = self.program.statements.len();
            self._parse_line(&ll);
            if self.program.hash_cut.is_none()
                && self.program.statements[before..]
                    .iter()
                    .any(|st| hash_statement(st).is_some())
            {
                self.program.hash_cut = Some(self.diags.items.len());
            }
            if self.done {
                break;
            }
        }
        for ll in todo {
            let extra = if ll.unpulled.is_some() { ll.pulls } else { 0 };
            let shift = unshift.get(&ll.file).copied().unwrap_or(0) + extra;
            if let Some(ref unpulled) = ll.unpulled {
                for mut l in unpulled.iter().cloned() {
                    l.line += shift;
                    self._note_line(&l);
                }
                *unshift.entry(ll.file.clone()).or_insert(0) += ll.pulls;
            } else {
                let mut l = ll;
                l.line += shift;
                self._note_line(&l);
            }
        }
        if !self.cond_stack.is_empty() {
            self.program.unclosed_if = true;
        }
        self.program.consts = self.env.consts.borrow().clone();
        (
            std::mem::take(&mut self.program),
            std::mem::replace(&mut self.diags, Diagnostics::new()),
        )
    }

    /// Port of `parser.py::_Parser._note_line`
    pub fn _note_line(&mut self, ll: &LogicalLine) {
        let Some((first, last)) = ll.phys else {
            return;
        };
        if self.collect || !self.provenance {
            return;
        }
        let span = LineSpan {
            first,
            last,
            occurrence: ll.occurrence,
        };
        if !self.line_map.contains_key(&ll.file) {
            self.line_map.insert(
                ll.file.clone(),
                std::rc::Rc::new(std::cell::RefCell::new(crate::syntax::OrderedMap::new())),
            );
        }
        if let Some(file_map_rc) = self.line_map.get(&ll.file) {
            let mut file_map = file_map_rc.borrow_mut();
            if !file_map.contains_key(&ll.line) {
                file_map.insert(
                    ll.line,
                    std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
                );
            }
            if let Some(spans_rc) = file_map.get(&ll.line) {
                let mut spans = spans_rc.borrow_mut();
                if !spans.contains(&span) {
                    spans.push(span);
                }
            }
        }
    }

    /// Port of `parser.py::_Parser._active`
    pub fn _active(&self) -> bool {
        self.cond_stack.iter().all(|f| f.current)
    }

    /// Port of `parser.py::_Parser._handle_directive`
    pub fn _handle_directive(&mut self, kind: &str, rest: &str, line: i64, file: &str) {
        match kind {
            "IF" => {
                let parent_ok = self._active();
                let cond = if parent_ok {
                    self._eval_directive_cond(rest, line, file)
                } else {
                    false
                };
                self.cond_stack.push(CondFrame {
                    any_true: cond,
                    current: cond,
                    parent_ok,
                });
            }
            "ELSEIF" => {
                let Some(frame) = self.cond_stack.last() else {
                    self.diags.error_basic(line, "#ElseIf without #If.", file);
                    return;
                };
                if !frame.parent_ok || frame.any_true {
                    if let Some(frame) = self.cond_stack.last_mut() {
                        frame.current = false;
                    }
                } else {
                    let cond = self._eval_directive_cond(rest, line, file);
                    if let Some(frame) = self.cond_stack.last_mut() {
                        frame.current = cond;
                        frame.any_true = frame.any_true || cond;
                    }
                }
            }
            "ELSE" => {
                let Some(frame) = self.cond_stack.last_mut() else {
                    self.diags.error_basic(line, "#Else without #If.", file);
                    return;
                };
                frame.current = frame.parent_ok && !frame.any_true;
                frame.any_true = frame.any_true || frame.current;
            }
            "ENDIF" => {
                if self.cond_stack.pop().is_none() {
                    self.diags.error_basic(line, "#EndIf without #If.", file);
                }
            }
            "UNDEF" => {
                let trimmed = pystr::strip(rest);
                let name = pystr::split_ws(trimmed, None)
                    .first()
                    .map_or_else(String::new, |w| pystr::upper(w));
                self.env.consts.borrow_mut().remove(&name);
            }
            _ => {}
        }
    }

    /// Port of `parser.py::_Parser._undefined_directive_names`
    pub fn _undefined_directive_names(&self, expr: &Expr) -> Vec<String> {
        match expr {
            Expr::Name(n) => {
                if consteval::value_of(Some(expr), &self.env).is_ok() {
                    Vec::new()
                } else {
                    vec![n.name.clone()]
                }
            }
            Expr::Binary(b) => {
                let mut out = Vec::new();
                if let Some(ref l) = b.left {
                    out.extend(self._undefined_directive_names(l));
                }
                if let Some(ref r) = b.right {
                    out.extend(self._undefined_directive_names(r));
                }
                out
            }
            Expr::Unary(u) => {
                if let Some(ref op) = u.operand {
                    self._undefined_directive_names(op)
                } else {
                    Vec::new()
                }
            }
            Expr::Index(ix) => ix
                .args
                .iter()
                .flatten()
                .flat_map(|a| self._undefined_directive_names(a))
                .collect(),
            Expr::Member(m) => {
                let mut out = Vec::new();
                if let Some(ref b) = m.base {
                    out.extend(self._undefined_directive_names(b));
                }
                if let Some(ref args) = m.args {
                    for a in args.iter().flatten() {
                        out.extend(self._undefined_directive_names(a));
                    }
                }
                out
            }
            Expr::ArrayLiteral(a) => a
                .items
                .iter()
                .flatten()
                .flat_map(|it| self._undefined_directive_names(it))
                .collect(),
            Expr::Juxtaposed(a) => a
                .items
                .iter()
                .flatten()
                .flat_map(|it| self._undefined_directive_names(it))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Port of `parser.py::_Parser._eval_directive_cond`
    pub fn _eval_directive_cond(&mut self, rest: &str, line: i64, file: &str) -> bool {
        let mut echo = pystr::strip(rest).to_string();
        if let Some(m) = _THEN_WORD_RE.search(&echo)
            && !pystr::strip(pystr::slice(
                &echo,
                None,
                Some(i64::try_from(m.start(0).unwrap_or(0)).unwrap_or(0)),
            ))
            .is_empty()
        {
            echo = pystr::slice(
                &echo,
                None,
                Some(i64::try_from(m.start(0).unwrap_or(0)).unwrap_or(0)),
            )
            .to_string();
        }
        let text = pystr::strip(&echo).to_string();
        if text.is_empty() {
            self.diags
                .error_basic(line, "Invalid expression in #If statement.", file);
            return false;
        }
        let parsed = tokenize_for_parser(&text, line)
            .map_err(|_| ())
            .and_then(|tokens| {
                let mut p = TokStream::new(tokens, "");
                self._parse_expr(&mut p, line, file)
                    .map(|expr| (expr, p))
                    .map_err(|_| ())
            });
        let Ok((expr, p)) = parsed else {
            let open = text.matches('(').count();
            let close = text.matches(')').count();
            let verb = if open > close {
                "No Matching, or out of place ) in expression: "
            } else {
                "Invalid, or out of place expression: "
            };
            self.diags
                .error_basic(line, &format!("{verb}{echo}."), file);
            return false;
        };
        if !(p.at_end() || p.is_op(":", 0) || p.is_ident("THEN", 0)) {
            self.diags
                .error_basic(line, &format!("Incomplete expression: {echo}."), file);
            return false;
        }
        if let Ok(value) = consteval::value_of(Some(&expr), &self.env) {
            consteval::truthy(&value)
        } else {
            if self.profile.single_pass {
                for name in self._undefined_directive_names(&expr) {
                    let msg = self.profile.unresolved_symbol_message.format(&name);
                    self.diags.error_basic(line, &msg, file);
                }
            } else {
                self.diags.error_basic(
                    line,
                    &format!("Invalid symbol in expression: {echo}."),
                    file,
                );
            }
            false
        }
    }

    /// Port of `parser.py::_Parser._parse_line`
    pub fn _parse_line(&mut self, ll: &LogicalLine) {
        let segments = _split_text_segments(&ll.text);
        let last_idx = segments.len().saturating_sub(1);
        let mut seg_offsets = vec![0i64];
        for seg in &segments {
            let prev = *seg_offsets.last().unwrap_or(&0);
            let char_len = i64::try_from(seg.chars().count()).unwrap_or(0);
            seg_offsets.push(prev + char_len + 1);
        }
        let mut in_inline_if = false;
        let mut colon_if = false;
        for (idx, seg_text_raw) in segments.iter().enumerate() {
            let trimmed = pystr::strip(seg_text_raw);
            if trimmed.is_empty() || trimmed == "=" {
                continue;
            }
            if colon_if && _COLON_IF_END_RE.fullmatch(seg_text_raw).is_some() {
                continue;
            }
            let at_eol = idx == last_idx && !ll.had_trailing_comment && !in_inline_if;
            let word = _LEADING_WORD_RE.match_(seg_text_raw);
            let mut stmt = if let Some(ref m) = word
                && pystr::upper(m.group(1).unwrap_or("")) == "UNITS"
            {
                self._parse_units_segment(seg_text_raw, ll.line, &ll.file)
            } else {
                self._parse_text_segment(seg_text_raw, ll.line, &ll.file)
            };
            if self._folded_include_error(&stmt) {
                continue;
            }
            stmt.base().at_eol.set(at_eol && !stmt.base().grouped.get());
            if self.provenance {
                *stmt.base().origin.borrow_mut() = Some((
                    Rc::new(ll.clone()),
                    usize::try_from(seg_offsets[idx]).unwrap_or(0),
                ));
            }
            if let Stmt::If(ref mut if_stmt) = stmt
                && if_stmt.inline.is_none()
                && idx < last_idx
                && matches!(self.profile.name, "cr6" | "cr1000x")
                && _COLON_IF_THEN_RE.search(seg_text_raw).is_some()
            {
                if_stmt.inline = Some(Vec::new());
                colon_if = true;
            }
            let has_inline = matches!(&stmt, Stmt::If(if_s) if if_s.inline.is_some());
            in_inline_if = in_inline_if || has_inline;
            let hoisted = if let Stmt::If(ref mut if_stmt) = stmt
                && if_stmt.inline.is_some()
            {
                _hoist_block_statements(if_stmt)
            } else {
                Vec::new()
            };
            let bad_cond_body = if let Stmt::Unparsed(ref u) = stmt
                && self.profile.single_pass
            {
                self._inline_body_after_bad_condition(u, seg_text_raw)
            } else {
                Vec::new()
            };
            let is_endprog = matches!(&stmt, Stmt::Instruction(inst) if inst.keyword == "ENDPROG");
            self.program.statements.push(Rc::new(stmt));
            for h in hoisted {
                self.program.statements.push(h);
            }
            for b in bad_cond_body {
                self.program.statements.push(Rc::new(b));
            }
            if is_endprog {
                self.done = true;
                return;
            }
        }
    }

    /// Port of `parser.py::_Parser._folded_include_error`
    pub fn _folded_include_error(&mut self, stmt: &Stmt) -> bool {
        let Stmt::Instruction(inst) = stmt else {
            return false;
        };
        if !matches!(inst.keyword.as_str(), "INCLUDE" | "INCLUDESECTION") {
            return false;
        }
        let index = usize::from(inst.keyword == "INCLUDESECTION");
        let Some(Some(arg)) = inst.args.get(index) else {
            return false;
        };
        let value = match consteval::value_of(Some(arg), &self.env) {
            Ok(v) => v,
            Err(_) => {
                if let Expr::Name(n) = arg.as_ref() {
                    let up = pystr::upper(&n.name);
                    if up == "NAN" {
                        ConstValue::Str("-nan".to_string())
                    } else if up == "INF" {
                        ConstValue::Str("inf".to_string())
                    } else {
                        return self._runtime_include_name(inst, index);
                    }
                } else {
                    return self._runtime_include_name(inst, index);
                }
            }
        };
        if inst.args.len() != index + 1 {
            return false;
        }
        let spec = match value {
            ConstValue::Str(s) => s,
            ConstValue::Int(mut iv) => {
                if let Expr::Number(num) = arg.as_ref() {
                    let up = pystr::upper(&num.text);
                    if (up.starts_with("&H") || up.starts_with("&B"))
                        && ((1i64 << 31)..(1i64 << 32)).contains(&iv)
                    {
                        iv -= 1i64 << 32;
                    }
                }
                if consteval::is_float(Some(arg), &self.env).unwrap_or(false) {
                    i32::try_from(iv)
                        .map_or_else(|_| iv.to_string(), |n| pystr::format_g(f64::from(n), 7))
                } else {
                    if let Expr::Binary(_) = arg.as_ref()
                        && let Some(converted) = _long_value_fallback(arg, &self.env)
                    {
                        iv = converted;
                    }
                    if !(-(1i64 << 31)..(1i64 << 31)).contains(&iv) {
                        return false;
                    }
                    if iv == -(1i64 << 31)
                        && (!self.profile.single_pass || matches!(arg.as_ref(), Expr::Binary(_)))
                    {
                        "NAN".to_string()
                    } else if !self.profile.single_pass {
                        iv.to_string()
                    } else {
                        let n = i32::try_from(iv).unwrap_or(0);
                        pystr::format_g(f64::from(n), 7)
                    }
                }
            }
            ConstValue::Float(fv) | ConstValue::CFloat(fv) => {
                if consteval::is_float(Some(arg), &self.env).unwrap_or(false) {
                    pystr::format_g(fv, 7)
                } else {
                    let Some(iv) = pystr::py_int_from_f64(fv) else {
                        return false;
                    };
                    if !(-(1i64 << 31)..(1i64 << 31)).contains(&iv) {
                        return false;
                    }
                    if iv == -(1i64 << 31)
                        && (!self.profile.single_pass || matches!(arg.as_ref(), Expr::Binary(_)))
                    {
                        "NAN".to_string()
                    } else if !self.profile.single_pass {
                        iv.to_string()
                    } else {
                        pystr::format_g(fv, 7)
                    }
                }
            }
        };
        let bad = crate::source::bad_include_extension(
            &spec,
            _include_own_ext(self.profile.name),
            _cr3xx_include_exts(self.profile.name).as_deref(),
        );
        if bad {
            self.diags.error_basic(
                inst.base.node.line,
                &format!("Include file {spec} must have valid program file extension."),
                &inst.base.node.file,
            );
            self.program
                .include_error_at
                .push(self.program.statements.len());
        } else if self.profile.single_pass && !_matches_cr3xx_or_crb(&spec, self.profile.name) {
            self.diags.warning_basic(
                inst.base.node.line,
                &format!("Cannot open include file  {spec}."),
                &inst.base.node.file,
            );
        } else {
            return false;
        }
        true
    }

    /// Port of `parser.py::_Parser._runtime_include_name`
    pub fn _runtime_include_name(&mut self, inst: &Instruction, index: usize) -> bool {
        let mut declared: std::collections::HashSet<String> = std::collections::HashSet::new();
        for st in &self.program.statements {
            _collect_declared_names(st, &mut declared);
        }
        let mut names: Vec<(String, bool)> = Vec::new();
        if let Some(Some(arg)) = inst.args.get(index) {
            _walk_expr_names(arg, &mut names);
        }
        let consts_upper: std::collections::HashSet<String> = self
            .env
            .consts
            .borrow()
            .keys()
            .map(|k| pystr::upper(k))
            .collect();
        let is_known = |up: &str| -> bool {
            declared.contains(up)
                || consts_upper.contains(up)
                || self.profile.symbols().contains(up)
                || self.profile.functions().contains(up)
        };

        let undeclared: Vec<String> = names
            .iter()
            .filter(|(n, _)| !is_known(&pystr::upper(n)))
            .map(|(n, _)| n.clone())
            .collect();
        let runtime = names.iter().any(|(n, call)| {
            let up = pystr::upper(n);
            is_known(&up) && !consts_upper.contains(&up) && (declared.contains(&up) || *call)
        });
        let text = pystr::strip(_arg_text_fallback(inst, index)).to_string();
        if undeclared.is_empty() && !runtime {
            return false;
        }
        if self.profile.single_pass {
            for n in &undeclared {
                self.diags.error_basic(
                    inst.base.node.line,
                    &format!("{n} is not defined."),
                    &inst.base.node.file,
                );
            }
            self.diags.error_basic(
                inst.base.node.line,
                &format!("Variable illegal in Parameter: {text}."),
                &inst.base.node.file,
            );
            self.diags.warning_basic(
                inst.base.node.line,
                "Cannot open include file  .",
                &inst.base.node.file,
            );
            return true;
        }
        if !undeclared.is_empty() {
            self.diags.error_basic(
                inst.base.node.line,
                &format!("Invalid symbol in expression: {text}."),
                &inst.base.node.file,
            );
        }
        if runtime {
            self.diags.error_basic(
                inst.base.node.line,
                &format!("Variable illegal in Parameter: {text}."),
                &inst.base.node.file,
            );
        }
        self.diags.error_basic(
            inst.base.node.line,
            "Include file NULL must have valid program file extension.",
            &inst.base.node.file,
        );
        self.program
            .include_error_at
            .push(self.program.statements.len());
        true
    }

    /// Port of `parser.py::_Parser._inline_body_after_bad_condition`
    pub fn _inline_body_after_bad_condition(
        &mut self,
        stmt: &Unparsed,
        seg_text_raw: &str,
    ) -> Vec<Stmt> {
        let Some(m) = _BAD_COND_INLINE_RE.match_(seg_text_raw) else {
            return Vec::new();
        };
        if !stmt.reason.starts_with("expected THEN") {
            return Vec::new();
        }
        let probe_text = format!("If 1 Then{}", m.group(1).unwrap_or(""));
        let probe =
            self._parse_text_segment(&probe_text, stmt.base.node.line, &stmt.base.node.file);
        let Stmt::If(if_stmt) = probe else {
            return Vec::new();
        };
        let Some(inline) = if_stmt.inline else {
            return Vec::new();
        };
        if inline.is_empty() {
            return Vec::new();
        }
        let mut body: Vec<Stmt> = inline.into_iter().map(|rc| (*rc).clone()).collect();
        if let Some(inline_else) = if_stmt.inline_else {
            body.extend(inline_else.into_iter().map(|rc| (*rc).clone()));
        }
        for st in &mut body {
            st.base().at_eol.set(false);
        }
        body
    }

    /// Port of `parser.py::_Parser._parse_text_segment`
    pub fn _parse_text_segment(&mut self, seg_text_raw: &str, line: i64, file: &str) -> Stmt {
        let tokens = match tokenize_for_parser(seg_text_raw, line) {
            Ok(toks) => toks,
            Err(e) => {
                let e_col_i = i64::try_from(e.col).unwrap_or(0);
                let prefix = pystr::slice(seg_text_raw, None, Some(e_col_i));
                if e.message == "unterminated string" && _SUB_UNTERM_RE.fullmatch(prefix).is_some()
                {
                    return self._parse_text_segment(prefix, line, file);
                }
                if _ESS_INIT_RE.match_(seg_text_raw).is_some() {
                    return Stmt::Unparsed(Unparsed {
                        base: _stmt_base(line, file, pystr::strip(seg_text_raw)),
                        reason: "empty statement".to_string(),
                        expr: None,
                    });
                }
                let m = _LEAD_IDENT_RE.match_(seg_text_raw);
                let lead_upper = m
                    .as_ref()
                    .map(|m| pystr::upper(m.group(1).unwrap_or("")))
                    .unwrap_or_default();
                let rec = if m.is_some() {
                    self.profile.instructions().get(lead_upper.as_str())
                } else {
                    None
                };
                let real: Vec<i64> = rec
                    .map(|r| {
                        r.params
                            .iter()
                            .copied()
                            .filter(|c| !matches!(c, 14 | 32 | 36 | 46))
                            .collect()
                    })
                    .unwrap_or_default();
                let slow = !real.is_empty() && lead_upper == "SLOWSEQUENCE";
                let slow_shape = _SLOW_SHAPE1_RE.fullmatch(prefix).is_some()
                    || _SLOW_SHAPE2_RE.fullmatch(prefix).is_some();
                if e.message == "unterminated string"
                    && !real.is_empty()
                    && real[0] >= 100
                    && (!slow || slow_shape)
                    && pystr::slice(seg_text_raw, Some(e_col_i), Some(e_col_i + 1)) == "\""
                {
                    if slow && prefix.contains(',') {
                        let mut stmt = self._parse_text_segment(&format!("{prefix}0)"), line, file);
                        if let Stmt::Instruction(ref mut inst) = stmt {
                            inst.base.text = seg_text_raw.to_string();
                        }
                        return stmt;
                    }
                    let stripped = pystr::rstrip_chars(pystr::rstrip(prefix), ",");
                    return self._parse_text_segment(stripped, line, file);
                }
                if let Some(m) = _CONST_BRACKET_RE.match_(seg_text_raw) {
                    let name = m.group(2).unwrap_or("");
                    let typ = m.group(3).unwrap_or("");
                    if !seg_text_raw.contains('=') {
                        return self._parse_text_segment(&format!("Const {name}"), line, file);
                    }
                    self.env.define(name, ConstValue::Int(0), Some(false));
                    return Stmt::ConstDecl(ConstDecl {
                        base: _stmt_base(line, file, seg_text_raw),
                        name: name.to_string(),
                        value: Some(Rc::new(Expr::Number(Number {
                            node: Node::new(line, file),
                            value: NumberValue::Int(0),
                            text: "0".to_string(),
                        }))),
                        type_: Some(typ.to_string()),
                        value_span: Some((0, 0)),
                    });
                }
                if e.message == "unterminated string"
                    && _CONST_QUOTE_RE.match_(seg_text_raw).is_some()
                {
                    let rest_after_col = pystr::slice(seg_text_raw, Some(e_col_i), None);
                    let word = _CONST_NON_SPACE_COMMA_RE
                        .match_(rest_after_col)
                        .and_then(|m| m.group(0))
                        .unwrap_or("");
                    let reason = if seg_text_raw.contains('=') {
                        format!("Const lead: Invalid variable or constant name: {word}.")
                    } else {
                        format!(
                            "Const lead: Const with no assignment.\nToo many parameters:  {word}."
                        )
                    };
                    return Stmt::Unparsed(Unparsed {
                        base: _stmt_base(line, file, ""),
                        reason,
                        expr: None,
                    });
                }
                if e.message == "unterminated string"
                    && !seg_text_raw.contains('=')
                    && _CONST_NUM_BEFORE_QUOTE_RE.fullmatch(prefix).is_some()
                {
                    return self._parse_text_segment(pystr::rstrip(prefix), line, file);
                }
                if e.message == "unterminated string"
                    && _CONST_IDENT_RE.match_(seg_text_raw).is_some()
                    && let Some(stmt) = self._const_unterminated(seg_text_raw, e.col, line, file)
                {
                    return stmt;
                }
                let text = if e.message == "unterminated string" {
                    seg_text_raw
                } else {
                    pystr::strip(seg_text_raw)
                };
                return Stmt::Unparsed(Unparsed {
                    base: _stmt_base(line, file, text),
                    reason: format!("lex error: {}", e.message),
                    expr: None,
                });
            }
        };
        let real: Vec<&Token> = tokens.iter().filter(|t| t.kind != "EOF").collect();
        if real.is_empty()
            || real
                .iter()
                .all(|t| t.kind == "OP" && (t.text == "(" || t.text == ")" || t.text == ","))
        {
            return Stmt::Unparsed(Unparsed {
                base: _stmt_base(line, file, pystr::strip(seg_text_raw)),
                reason: "empty statement".to_string(),
                expr: None,
            });
        }
        let seg_text = Self::_segment_text(seg_text_raw, &real);
        let tokens = super::helpers::_drop_semicolons(tokens, self.profile);
        let closed = super::helpers::_close_open_call(tokens, self.profile);
        let dropped = closed.dropped;
        let stmt = self._parse_segment(closed.tokens, line, file, seg_text);
        if let Some(d) = dropped {
            *stmt.base().dropped_tail.borrow_mut() = Some(d);
        }
        stmt
    }

    /// Port of `parser.py::_Parser._segment_text`
    pub fn _segment_text<'a>(line_text: &'a str, _real_tokens: &[&Token]) -> &'a str {
        line_text
    }

    /// Port of `parser.py::_Parser._cr3xx_argument_semicolon`
    pub fn _cr3xx_argument_semicolon(
        &mut self,
        mut seg: Vec<Token>,
        line: i64,
        file: &str,
        seg_text: &str,
    ) -> Vec<Token> {
        if seg.len() < 4 || seg[0].kind != "IDENT" || seg[1].text != "(" {
            return seg;
        }
        let Some(rec) = self.profile.instructions().get(seg[0].upper.as_str()) else {
            return seg;
        };
        if rec.params.first().copied() == Some(7) {
            return seg;
        }
        let mut depth: i64 = 1;
        let mut index: usize = 0;
        let mut start: usize = 2;
        let mut found_i: Option<usize> = None;
        for (i, t) in seg.iter().enumerate().skip(2) {
            if t.kind != "OP" {
                continue;
            }
            if t.text == "(" {
                depth += 1;
            } else if t.text == ")" {
                depth -= 1;
                if depth == 0 {
                    return seg;
                }
            } else if t.text == "," && depth == 1 {
                index += 1;
                start = i + 1;
            } else if t.text == ";" && depth == 1 && i > start {
                found_i = Some(i);
                break;
            }
        }
        let Some(i) = found_i else {
            return seg;
        };
        let t_col = seg[i].col;
        let table = seg[0].upper == "DATATABLE";
        let symbol = if table && index == 0 && i == 3 && seg[2].kind == "IDENT" {
            let seg2_len = i64::try_from(seg[2].text.chars().count()).unwrap_or(0);
            if t_col == seg[2].col + seg2_len {
                return seg;
            }
            index = 1;
            ";".to_string()
        } else {
            pystr::strip(pystr::slice(
                seg_text,
                Some(seg[start].col),
                Some(t_col + 1),
            ))
            .to_string()
        };
        self.diags.error_basic(
            line,
            &format!("Invalid symbol in expression: {symbol}."),
            file,
        );
        let params = &rec.params;
        if index < params.len() && _ABORTING_PARAMS.contains(&params[index]) {
            let mut depth: i64 = 1;
            for u in seg.iter().skip(i + 1) {
                if u.kind == "OP" && (u.text == "(" || u.text == ")") {
                    depth += if u.text == "(" { 1 } else { -1 };
                }
                if u.kind == "OP" && u.text == "," && depth == 1 {
                    let after = pystr::slice(seg_text, Some(u.col + 1), None);
                    if let Some(m) = _CR3XX_SEMI_NEXT_RE.match_(after) {
                        self.diags.error_basic(
                            line,
                            &format!("Too many parameters:  {}.", m.group(1).unwrap_or("")),
                            file,
                        );
                    }
                    break;
                }
                if depth == 0 {
                    break;
                }
            }
            let mut depth: i64 = 0;
            let mut commas: usize = 0;
            let mut cut_j: Option<(usize, usize)> = None;
            for (j, u) in seg.iter().enumerate() {
                if u.kind == "OP" && (u.text == "(" || u.text == ")") {
                    depth += if u.text == "(" { 1 } else { -1 };
                } else if u.kind == "OP" && u.text == "," && depth == 1 {
                    commas += 1;
                    if commas == params.len()
                        && let Some(close) = (j + 1..seg.len())
                            .rev()
                            .find(|&k| seg[k].kind == "OP" && seg[k].text == ")")
                    {
                        cut_j = Some((j, close));
                        break;
                    }
                }
            }
            if let Some((j, close)) = cut_j {
                let mut new_seg = seg[..j].to_vec();
                new_seg.extend_from_slice(&seg[close..]);
                seg = new_seg;
            }
        }
        let mut out = Vec::with_capacity(seg.len());
        let mut removed = false;
        for u in seg {
            if !removed && u.kind == "OP" && u.text == ";" && u.col == t_col {
                removed = true;
                continue;
            }
            out.push(u);
        }
        out
    }

    /// Port of `parser.py::_Parser._grouped_statement`
    pub fn _grouped_statement(
        &mut self,
        seg: &[Token],
        line: i64,
        file: &str,
        seg_text: &str,
    ) -> Option<Stmt> {
        let real: Vec<Token> = seg.iter().filter(|t| t.kind != "EOF").cloned().collect();
        let recover = self.profile.single_pass && self.inline_body;
        if !(real.len() > 2
            && real[0].kind == "OP"
            && real[0].text == "("
            && (recover || (real.last()?.kind == "OP" && real.last()?.text == ")")))
        {
            return None;
        }
        let mut depth: i64 = 0;
        let mut close = real.len();
        for (k, t) in real.iter().enumerate() {
            if t.kind == "OP" && (t.text == "(" || t.text == ")") {
                depth += if t.text == "(" { 1 } else { -1 };
                if depth == 0 {
                    close = k;
                    break;
                }
            }
        }
        if !recover && close != real.len() - 1 {
            return None;
        }
        let inner_text = if seg_text.is_empty() {
            real[1..close]
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            let start = seg_text.chars().position(|c| c == '(').unwrap_or(0);
            let start_i = i64::try_from(start).unwrap_or(0);
            let end_i = if close < real.len() {
                start_i + real[close].col - real[0].col
            } else {
                i64::try_from(seg_text.chars().count()).unwrap_or(0)
            };
            pystr::strip(pystr::slice(seg_text, Some(start_i + 1), Some(end_i))).to_string()
        };
        let kept = self.diags.items.len();
        let mut inner_seg = real[1..close].to_vec();
        inner_seg.extend(seg.iter().filter(|t| t.kind == "EOF").cloned());
        let mut inner = self._parse_segment(inner_seg, line, file, seg_text);
        if matches!(&inner, Stmt::Assignment(_) | Stmt::Instruction(_)) {
            if self.profile.single_pass {
                inner.base_mut().text = inner_text;
                inner.base().grouped.set(true);
            }
            return Some(inner);
        }
        self.diags.items.truncate(kept);
        None
    }

    /// Port of `parser.py::_Parser._parse_segment`
    pub fn _parse_segment(
        &mut self,
        mut seg: Vec<Token>,
        line: i64,
        file: &str,
        seg_text: &str,
    ) -> Stmt {
        if let Some(grouped) = self._grouped_statement(&seg, line, file, seg_text) {
            return grouped;
        }
        if self.profile.single_pass {
            seg = self._cr3xx_argument_semicolon(seg, line, file, seg_text);
        }
        if seg.len() > 3
            && seg[0].upper == "DATATABLE"
            && seg[1].text == "("
            && seg[2].kind == "IDENT"
            && seg[3].kind == "OP"
            && seg[3].text == ";"
        {
            let seg2_len = i64::try_from(seg[2].text.chars().count()).unwrap_or(0);
            if seg[3].col == seg[2].col + seg2_len {
                let name = format!("{};", seg[2].text);
                let mut new_seg = seg[..2].to_vec();
                new_seg.push(Token {
                    kind: "IDENT".to_string(),
                    text: name.clone(),
                    upper: pystr::upper(&name),
                    col: seg[2].col,
                    line: seg[2].line,
                    value: TokenValue::None,
                });
                new_seg.extend_from_slice(&seg[4..]);
                seg = new_seg;
            } else {
                self.diags
                    .error_basic(line, "Invalid, or out of place expression: ;.", file);
                let mut new_seg = seg[..3].to_vec();
                new_seg.push(Token {
                    kind: "OP".to_string(),
                    text: ",".to_string(),
                    upper: ",".to_string(),
                    col: seg[3].col,
                    line: seg[3].line,
                    value: TokenValue::None,
                });
                new_seg.push(Token {
                    kind: "NUMBER".to_string(),
                    text: "0".to_string(),
                    upper: "0".to_string(),
                    col: seg[3].col,
                    line: seg[3].line,
                    value: TokenValue::Int(0),
                });
                new_seg.extend_from_slice(&seg[4..]);
                seg = new_seg;
            }
        }
        if seg.len() > 3
            && seg[0].kind == "IDENT"
            && _RAW_NAME_FIRST.contains(&seg[0].upper.as_str())
            && seg[1].kind == "OP"
            && seg[1].text == "("
            && seg[2].kind == "IDENT"
            && seg[3].kind == "IDENT"
        {
            let mut new_seg = seg[..3].to_vec();
            new_seg.push(Token {
                kind: "OP".to_string(),
                text: ",".to_string(),
                upper: ",".to_string(),
                col: seg[3].col,
                line: seg[3].line,
                value: TokenValue::None,
            });
            new_seg.extend_from_slice(&seg[3..]);
            seg = new_seg;
        }
        if seg.len() > 2
            && seg[0].kind == "IDENT"
            && seg[0].upper == "CALLTABLE"
            && (seg[1].kind == "NUMBER"
                || (seg[1].kind == "OP"
                    && seg[1].text == "("
                    && seg.len() > 3
                    && seg[2].kind == "NUMBER"))
        {
            let at = if seg[1].kind == "NUMBER" { 1 } else { 2 };
            let at_col = usize::try_from(seg[at].col.max(0)).unwrap_or(0);
            if let Some(m) = _DIGIT_NAME.match_at(seg_text, at_col) {
                let end = i64::try_from(m.end(0).unwrap_or(0)).unwrap_or(0);
                let mut new_seg = seg[..at].to_vec();
                let name = m.group(0).unwrap_or("").to_string();
                new_seg.push(Token {
                    kind: "IDENT".to_string(),
                    text: name.clone(),
                    upper: pystr::upper(&name),
                    col: seg[at].col,
                    line: seg[at].line,
                    value: TokenValue::None,
                });
                new_seg.extend(seg[at + 1..].iter().filter(|t| t.col >= end).cloned());
                seg = new_seg;
            }
        }
        if seg.len() > 3
            && seg[0].kind == "IDENT"
            && seg[0].upper == "DATATABLE"
            && seg[1].kind == "OP"
            && seg[1].text == "("
            && matches!(seg[2].kind.as_str(), "NUMBER" | "STRING")
        {
            let col_u = usize::try_from(seg[2].col.max(0)).unwrap_or(0);
            let matched = if seg[2].kind == "STRING"
                && let TokenValue::Str(ref s) = seg[2].value
                && _DIGIT_NAME.fullmatch(s).is_some()
            {
                let end = seg[2].col + i64::try_from(seg[2].text.chars().count()).unwrap_or(0);
                Some((s.clone(), end))
            } else if seg[2].kind == "NUMBER"
                && let Some(m) = _DIGIT_NAME.match_at(seg_text, col_u)
            {
                Some((
                    m.group(0).unwrap_or("").to_string(),
                    i64::try_from(m.end(0).unwrap_or(0)).unwrap_or(0),
                ))
            } else {
                None
            };
            if let Some((name, end)) = matched {
                let mut new_seg = seg[..2].to_vec();
                new_seg.push(Token {
                    kind: "IDENT".to_string(),
                    text: name.clone(),
                    upper: pystr::upper(&name),
                    col: seg[2].col,
                    line: seg[2].line,
                    value: TokenValue::None,
                });
                new_seg.extend(seg[3..].iter().filter(|t| t.col >= end).cloned());
                seg = new_seg;
            }
        }
        let mut p = TokStream::new(seg.clone(), seg_text);
        let parsed: Result<Option<Stmt>, ParseError> = if self.in_struct
            && !seg.is_empty()
            && seg[0].kind == "IDENT"
            && !self.profile.instructions().contains(seg[0].upper.as_str())
        {
            self._parse_vardecl(&mut p, line, file).and_then(|first| {
                let mut vars = vec![first];
                while p.is_op(",", 0) {
                    p.next();
                    if p.at_end() {
                        break;
                    }
                    vars.push(self._parse_vardecl(&mut p, line, file)?);
                }
                Ok(Some(Stmt::Declaration(Declaration {
                    base: StmtBase::new(line, file),
                    kind: "STRUCTUREMEMBER".to_string(),
                    vars,
                })))
            })
        } else {
            self._parse_stmt(&mut p, line, file, seg_text)
        };
        match parsed {
            Ok(stmt_opt) => {
                if let Some(Stmt::Instruction(ref inst)) = stmt_opt {
                    if matches!(inst.keyword.as_str(), "STRUCTURETYPE" | "ENDSTRUCTURETYPE") {
                        self.in_struct = inst.keyword == "STRUCTURETYPE";
                    }
                    if matches!(inst.keyword.as_str(), "DATATABLE" | "ENDTABLE") {
                        self.in_table = inst.keyword == "DATATABLE";
                    }
                }
                let Some(mut stmt) = stmt_opt else {
                    return Stmt::Unparsed(Unparsed {
                        base: _stmt_base(line, file, seg_text),
                        reason: "empty statement".to_string(),
                        expr: None,
                    });
                };
                if !p.at_end() {
                    let extra = p.peek(0);
                    let label = if extra.text.is_empty() {
                        &extra.kind
                    } else {
                        &extra.text
                    };
                    return Stmt::Unparsed(Unparsed {
                        base: _stmt_base(line, file, seg_text),
                        reason: format!(
                            "unexpected trailing input starting at {}",
                            pystr::repr_str(label)
                        ),
                        expr: None,
                    });
                }
                stmt.base_mut().text = seg_text.to_string();
                stmt
            }
            Err(e) => {
                if _SNMP_ERR_RE.match_(&e.0).is_some()
                    && _SNMP_OID_STATEMENT.match_(seg_text).is_some()
                {
                    return Stmt::Unparsed(Unparsed {
                        base: _stmt_base(line, file, seg_text),
                        reason: SNMP_OID_REASON.to_string(),
                        expr: None,
                    });
                }
                if let Some(placed) = self._table_only_outside_table(&seg, &p, line, file, seg_text)
                {
                    return Stmt::Instruction(placed);
                }
                Stmt::Unparsed(Unparsed {
                    base: _stmt_base(line, file, seg_text),
                    reason: e.0,
                    expr: None,
                })
            }
        }
    }

    /// Port of `parser.py::_Parser._table_only_outside_table`
    pub fn _table_only_outside_table(
        &mut self,
        seg: &[Token],
        p: &TokStream,
        line: i64,
        file: &str,
        seg_text: &str,
    ) -> Option<Instruction> {
        if self.in_table
            || self.in_struct
            || seg.len() < 4
            || seg[0].kind != "IDENT"
            || !_DEFERRED_SOURCE_INSTRUCTIONS.contains(&seg[0].upper.as_str())
            || !self.profile.instructions().contains(seg[0].upper.as_str())
            || seg[1].kind != "OP"
            || seg[1].text != "("
        {
            return None;
        }
        let mut depth: i64 = 0;
        let mut commas: Vec<usize> = Vec::new();
        for (i, tok) in seg.iter().enumerate().skip(1) {
            if tok.kind != "OP" {
                continue;
            }
            depth += i64::from(tok.text == "(") - i64::from(tok.text == ")");
            if depth < 0
                || (depth == 0 && tok.text == ")" && i + 1 < seg.len() && seg[i + 1].kind != "EOF")
            {
                return None;
            }
            if tok.text == "," && depth == 1 {
                commas.push(i);
            }
        }
        if depth != 0 || commas.len() < 2 || !(commas[0] < p.i && p.i <= commas[1]) {
            return None;
        }
        let mut sub_toks = seg[2..commas[0]].to_vec();
        sub_toks.push(super::helpers::_eof_like(seg[commas[0]].col));
        let mut sub = TokStream::new(sub_toks, seg_text);
        let first = self._parse_expr(&mut sub, line, file).ok()?;
        if !sub.at_end() {
            return None;
        }
        let mut args: Vec<Option<ExprRef>> = vec![Some(first)];
        for _ in &commas {
            args.push(Some(Rc::new(Expr::Number(Number {
                node: Node::new(line, file),
                value: NumberValue::Int(0),
                text: "0".to_string(),
            }))));
        }
        let arg_spans = vec![None; args.len()];
        Some(Instruction {
            base: _stmt_base(line, file, seg_text),
            keyword: seg[0].upper.clone(),
            args,
            arg_spans,
            extra_targets: Vec::new(),
            alias_junk: false,
            alias_item_junk: None,
        })
    }
}

fn _stmt_base(line: i64, file: &str, text: &str) -> StmtBase {
    let mut b = StmtBase::new(line, file);
    b.text = text.to_string();
    b
}

fn _matches_cr3xx_or_crb(spec: &str, profile_name: &str) -> bool {
    let up = pystr::upper(spec);
    if std::path::Path::new(&up)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("CRB"))
    {
        return true;
    }
    if let Some(exts) = _cr3xx_include_exts(profile_name) {
        return exts.iter().any(|e| up.ends_with(e));
    }
    false
}

fn _long_value_fallback(_expr: &Expr, _env: &consteval::Env) -> Option<i64> {
    // TODO(P2): delegate to checker::longconv::long_value once checker is ported
    None
}

fn _collect_declared_names(stmt: &Stmt, out: &mut std::collections::HashSet<String>) {
    match stmt {
        Stmt::Declaration(d) => {
            for v in &d.vars {
                out.insert(pystr::upper(&v.name));
            }
        }
        Stmt::ConstDecl(c) => {
            out.insert(pystr::upper(&c.name));
        }
        Stmt::SubDef(s) => {
            out.insert(pystr::upper(&s.name));
            for p in &s.params {
                out.insert(pystr::upper(&p.name));
            }
        }
        Stmt::Instruction(inst) => {
            if matches!(inst.keyword.as_str(), "DATATABLE" | "CONSTTABLE")
                && let Some(Some(arg0)) = inst.args.first()
                && let Expr::Name(n) = arg0.as_ref()
            {
                out.insert(pystr::upper(&n.name));
            }
        }
        _ => {}
    }
}

fn _walk_expr_names(expr: &Expr, out: &mut Vec<(String, bool)>) {
    match expr {
        Expr::Name(n) => out.push((n.name.clone(), false)),
        Expr::Index(ix) => {
            out.push((ix.name.clone(), true));
            for a in ix.args.iter().flatten() {
                _walk_expr_names(a, out);
            }
        }
        Expr::Binary(b) => {
            if let Some(ref l) = b.left {
                _walk_expr_names(l, out);
            }
            if let Some(ref r) = b.right {
                _walk_expr_names(r, out);
            }
        }
        Expr::Unary(u) => {
            if let Some(ref op) = u.operand {
                _walk_expr_names(op, out);
            }
        }
        Expr::Member(m) => {
            if let Some(ref b) = m.base {
                _walk_expr_names(b, out);
            }
            if let Some(ref args) = m.args {
                for a in args.iter().flatten() {
                    _walk_expr_names(a, out);
                }
            }
        }
        Expr::ArrayLiteral(a) => {
            for it in a.items.iter().flatten() {
                _walk_expr_names(it, out);
            }
        }
        Expr::Juxtaposed(a) => {
            for it in a.items.iter().flatten() {
                _walk_expr_names(it, out);
            }
        }
        _ => {}
    }
}

fn _arg_text_fallback(inst: &Instruction, index: usize) -> &str {
    // TODO(P2): delegate to checker::expressions::_arg_text once checker is ported
    if let Some(Some((start, end))) = inst.arg_spans.get(index) {
        pystr::slice(&inst.base.text, Some(*start), Some(*end))
    } else {
        ""
    }
}
