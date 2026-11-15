//! Port of open-crbasic `parser.py`: CRBasic parser (logical lines -> `syntax::Program`).

#![allow(dead_code, unused_imports, clippy::used_underscore_items)]

pub mod helpers;

use std::path::Path;

use crate::diagnostics::Diagnostics;
use crate::profiles::Profile;
use crate::syntax::{ExprRef, LogicalLine, Program, ProgramRef};

/// Port of `parser.py::_Parser`
pub struct Parser<'a> {
    pub provenance: bool,
    pub profile: &'a Profile,
    pub diags: Diagnostics,
    pub cond_stack: Vec<helpers::CondFrame>,
    pub program: Program,
    pub done: bool,
    pub inline_body: bool,
    pub in_struct: bool,
    pub in_table: bool,
    pub collect: bool,
    pub to_ok: bool,
    pub logical_assign: bool,
}

/// Port of `parser.py::_profile_for`
pub fn _profile_for(name: &str) -> &'static Profile {
    crate::profiles::for_file(Path::new(name))
}

/// Port of `parser.py::partial_expression`
pub fn partial_expression(_text: &str, _profile: &Profile) -> Option<ExprRef> {
    todo!("Chunk 4: partial_expression")
}

/// Port of `parser.py::parse_file`
pub fn parse_file(
    _path: &Path,
    _profile: Option<&Profile>,
    _pull_limit: Option<&[i64]>,
    _provenance: bool,
) -> Result<(ProgramRef, Diagnostics), std::io::Error> {
    todo!("Chunk 4: parse_file")
}

/// Port of `parser.py::_pulls_init`
pub fn _pulls_init(profile: &Profile) -> bool {
    matches!(profile.name, "cr6" | "cr1000x" | "cr300" | "cr350")
}

/// Port of `parser.py::parse_text`
pub fn parse_text(
    _text: &str,
    _name: &str,
    _directory: Option<&Path>,
    _profile: Option<&Profile>,
) -> (ProgramRef, Diagnostics) {
    todo!("Chunk 4: parse_text")
}
