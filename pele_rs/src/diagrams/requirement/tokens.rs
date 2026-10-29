#![allow(non_upper_case_globals)]
pub const END: usize = 0;
pub const ERROR: usize = 1;
pub const title: usize = 2;
pub const acc_title: usize = 3;
pub const acc_title_value: usize = 4;
pub const acc_descr: usize = 5;
pub const acc_descr_value: usize = 6;
pub const acc_descr_multiline_value: usize = 7;
pub const direction_tb: usize = 8;
pub const direction_bt: usize = 9;
pub const direction_rl: usize = 10;
pub const direction_lr: usize = 11;
pub const NEWLINE: usize = 12;
pub const EOF: usize = 13;
pub const RD: usize = 14;
pub const STRUCT_START: usize = 15;
pub const STRUCT_STOP: usize = 16;
pub const STYLE_SEPARATOR: usize = 17;
pub const COLONSEP: usize = 18;
pub const ID: usize = 19;
pub const TEXT: usize = 20;
pub const RISK: usize = 21;
pub const VERIFYMTHD: usize = 22;
pub const REQUIREMENT: usize = 23;
pub const FUNCTIONAL_REQUIREMENT: usize = 24;
pub const INTERFACE_REQUIREMENT: usize = 25;
pub const PERFORMANCE_REQUIREMENT: usize = 26;
pub const PHYSICAL_REQUIREMENT: usize = 27;
pub const DESIGN_CONSTRAINT: usize = 28;
pub const LOW_RISK: usize = 29;
pub const MED_RISK: usize = 30;
pub const HIGH_RISK: usize = 31;
pub const VERIFY_ANALYSIS: usize = 32;
pub const VERIFY_DEMONSTRATION: usize = 33;
pub const VERIFY_INSPECTION: usize = 34;
pub const VERIFY_TEST: usize = 35;
pub const ELEMENT: usize = 36;
pub const CONTAINS: usize = 37;
pub const COPIES: usize = 38;
pub const DERIVES: usize = 39;
pub const SATISFIES: usize = 40;
pub const VERIFIES: usize = 41;
pub const REFINES: usize = 42;
pub const TRACES: usize = 43;
pub const TYPE: usize = 44;
pub const DOCREF: usize = 45;
pub const STYLE: usize = 46;
pub const CLASSDEF: usize = 47;
pub const CLASS: usize = 48;
pub const ALPHA: usize = 49;
pub const COLON: usize = 50;
pub const SEMICOLON: usize = 51;
pub const PERCENT: usize = 52;
pub const MINUS: usize = 53;
pub const BRKT: usize = 54;
pub const END_ARROW_L: usize = 55;
pub const END_ARROW_R: usize = 56;
pub const LINE: usize = 57;
pub const qString: usize = 58;
pub const unqString: usize = 59;
pub const COMMA: usize = 60;
pub const TOKEN_NAMES: &[&str] = &[
    "$end",
    "INVALID",
    "title",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "NEWLINE",
    "EOF",
    "RD",
    "STRUCT_START",
    "STRUCT_STOP",
    "STYLE_SEPARATOR",
    "COLONSEP",
    "ID",
    "TEXT",
    "RISK",
    "VERIFYMTHD",
    "REQUIREMENT",
    "FUNCTIONAL_REQUIREMENT",
    "INTERFACE_REQUIREMENT",
    "PERFORMANCE_REQUIREMENT",
    "PHYSICAL_REQUIREMENT",
    "DESIGN_CONSTRAINT",
    "LOW_RISK",
    "MED_RISK",
    "HIGH_RISK",
    "VERIFY_ANALYSIS",
    "VERIFY_DEMONSTRATION",
    "VERIFY_INSPECTION",
    "VERIFY_TEST",
    "ELEMENT",
    "CONTAINS",
    "COPIES",
    "DERIVES",
    "SATISFIES",
    "VERIFIES",
    "REFINES",
    "TRACES",
    "TYPE",
    "DOCREF",
    "STYLE",
    "CLASSDEF",
    "CLASS",
    "ALPHA",
    "COLON",
    "SEMICOLON",
    "PERCENT",
    "MINUS",
    "BRKT",
    "END_ARROW_L",
    "END_ARROW_R",
    "LINE",
    "qString",
    "unqString",
    "COMMA",
];
pub fn keyword(s: &str) -> Option<usize> {
    match s {
        "requirementdiagram" => Some(RD),
        "id" => Some(ID),
        "text" => Some(TEXT),
        "risk" => Some(RISK),
        "verifymethod" => Some(VERIFYMTHD),
        "requirement" => Some(REQUIREMENT),
        "functionalrequirement" => Some(FUNCTIONAL_REQUIREMENT),
        "interfacerequirement" => Some(INTERFACE_REQUIREMENT),
        "performancerequirement" => Some(PERFORMANCE_REQUIREMENT),
        "physicalrequirement" => Some(PHYSICAL_REQUIREMENT),
        "designconstraint" => Some(DESIGN_CONSTRAINT),
        "low" => Some(LOW_RISK),
        "medium" => Some(MED_RISK),
        "high" => Some(HIGH_RISK),
        "analysis" => Some(VERIFY_ANALYSIS),
        "demonstration" => Some(VERIFY_DEMONSTRATION),
        "inspection" => Some(VERIFY_INSPECTION),
        "test" => Some(VERIFY_TEST),
        "element" => Some(ELEMENT),
        "contains" => Some(CONTAINS),
        "copies" => Some(COPIES),
        "derives" => Some(DERIVES),
        "satisfies" => Some(SATISFIES),
        "verifies" => Some(VERIFIES),
        "refines" => Some(REFINES),
        "traces" => Some(TRACES),
        "type" => Some(TYPE),
        "docref" => Some(DOCREF),
        "style" => Some(STYLE),
        "classdef" => Some(CLASSDEF),
        "class" => Some(CLASS),
        _ => None,
    }
}
