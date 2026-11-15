//! Port of `checker/instr_rules.py` (lines 1-3737).

#![allow(dead_code, unused_variables, unused_imports, clippy::doc_markdown)]

pub mod part1;
pub use part1::*;
pub mod part2;
pub use part2::*;
pub mod part3;
pub use part3::*;
pub mod part4;
pub use part4::*;
pub mod part5;
pub use part5::*;

pub struct RuleEntry {
    pub phase: &'static str,
    pub keywords: &'static [&'static str],
    pub single_pass: Option<bool>,
    pub func: RuleFn,
}

const PRECISION_VARIABLE_KEYWORDS: &[&str] = &["ADDPRECISE", "MOVEPRECISE"];
const SDM_ADDRESS_KEYWORDS: &[&str] = &[
    "CS7500",
    "CSAT3",
    "CSAT3B",
    "CSAT3BMONITOR",
    "EC100",
    "EC100CONFIGURE",
    "LI7200",
    "LI7700",
    "SDMCD16AC",
    "SDMCD16MASK",
    "SDMGENERIC",
    "SDMIO16",
    "SDMSIO4",
    "SDMSW8A",
    "SDMX50",
    "TDR100",
    "TDR200",
];
const RUNNING_AVG_KEYWORDS: &[&str] = &["AVGRUN", "TOTALRUN", "MINRUN", "MAXRUN", "STDDEVRUN"];

/// Port of `checker/instr_rules.py::RULES` (all 13 `@rule` entries in Python definition order).
pub static RULES: &[RuleEntry] = &[
    RuleEntry {
        phase: "pass1",
        keywords: &["MODEMCALLBACK"],
        single_pass: None,
        func: check_modemcallback_baud,
    },
    RuleEntry {
        phase: "pass1",
        keywords: PRECISION_VARIABLE_KEYWORDS,
        single_pass: None,
        func: check_precision_variable,
    },
    RuleEntry {
        phase: "pass1",
        keywords: &["VIBRATINGWIRE"],
        single_pass: None,
        func: check_vibrating_wire_diagnostics,
    },
    RuleEntry {
        phase: "tier_b",
        keywords: SDM_ADDRESS_KEYWORDS,
        single_pass: None,
        func: check_sdm_address,
    },
    RuleEntry {
        phase: "bounds",
        keywords: RUNNING_AVG_KEYWORDS,
        single_pass: None,
        func: check_avgrun_options,
    },
    RuleEntry {
        phase: "leading",
        keywords: &["SERIALBRK"],
        single_pass: None,
        func: check_serialbrk_comport,
    },
    RuleEntry {
        phase: "pass1",
        keywords: &["SERIALIN"],
        single_pass: None,
        func: check_serialin_destination,
    },
    RuleEntry {
        phase: "pass1",
        keywords: &["SERIALIN"],
        single_pass: None,
        func: check_serialin_comport,
    },
    RuleEntry {
        phase: "tier_b_generic",
        keywords: &["DEBUG"],
        single_pass: None,
        func: check_debug_args,
    },
    RuleEntry {
        phase: "bounds",
        keywords: &["SDI12RECORDER"],
        single_pass: Some(false),
        func: check_sdi12_nan_fill,
    },
    RuleEntry {
        phase: "bounds",
        keywords: &["PORTBRIDGE"],
        single_pass: Some(false),
        func: check_portbridge_array,
    },
    RuleEntry {
        phase: "late",
        keywords: &["PORTBRIDGE"],
        single_pass: Some(true),
        func: check_portbridge_array,
    },
    RuleEntry {
        phase: "bounds",
        keywords: &["MATRIX"],
        single_pass: Some(true),
        func: check_matrix_destination,
    },
];
