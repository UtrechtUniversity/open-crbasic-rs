//! A Rust implementation of [opencrbasic](https://github.com/UtrechtUniversity/open-crbasic),
//! the open re-implementation of Campbell Scientific's CRBasic compilers. In progress: see
//! `PORT_PROGRESS.md`.

pub mod checker;
pub mod compiler;
pub mod consteval;
pub mod diagnostics;
pub mod lexer;
pub mod mode;
pub mod parser;
pub mod preprocess;
pub mod profiles;
pub mod pyre;
pub mod pystr;
pub mod source;
pub mod sources;
pub mod syntax;
pub mod tables;
pub mod tdf;
