//! A Rust implementation of [opencrbasic](https://github.com/UtrechtUniversity/open-crbasic),
//! the open re-implementation of Campbell Scientific's CRBasic compilers. In progress: see
//! `PORT_PROGRESS.md`.

pub mod consteval;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod profiles;
pub mod pyre;
pub mod pystr;
pub mod syntax;
