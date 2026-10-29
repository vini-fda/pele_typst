#![allow(non_upper_case_globals)]
pub const END: usize = 0;
pub const ERROR: usize = 1;
pub const ER_DIAGRAM: usize = 2;
pub const EOF: usize = 3;
pub const NEWLINE: usize = 4;
pub const COLON: usize = 5;
pub const STYLE_SEPARATOR: usize = 6;
pub const BLOCK_START: usize = 7;
pub const BLOCK_STOP: usize = 8;
pub const SQS: usize = 9;
pub const SQE: usize = 10;
pub const acc_title: usize = 11;
pub const acc_title_value: usize = 12;
pub const acc_descr: usize = 13;
pub const acc_descr_value: usize = 14;
pub const acc_descr_multiline_value: usize = 15;
pub const END_KW: usize = 16;
pub const SUBGRAPH: usize = 17;
pub const direction_tb: usize = 18;
pub const direction_bt: usize = 19;
pub const direction_rl: usize = 20;
pub const direction_lr: usize = 21;
pub const CLASSDEF: usize = 22;
pub const UNICODE_TEXT: usize = 23;
pub const STYLE_TEXT: usize = 24;
pub const COMMA: usize = 25;
pub const CLASS: usize = 26;
pub const STYLE: usize = 27;
pub const SEMI: usize = 28;
pub const NUM: usize = 29;
pub const BRKT: usize = 30;
pub const ENTITY_NAME: usize = 31;
pub const DECIMAL_NUM: usize = 32;
pub const ENTITY_ONE: usize = 33;
pub const ATTRIBUTE_WORD: usize = 34;
pub const ATTRIBUTE_KEY: usize = 35;
pub const COMMENT: usize = 36;
pub const ZERO_OR_ONE: usize = 37;
pub const ZERO_OR_MORE: usize = 38;
pub const ONE_OR_MORE: usize = 39;
pub const ONLY_ONE: usize = 40;
pub const MD_PARENT: usize = 41;
pub const NON_IDENTIFYING: usize = 42;
pub const IDENTIFYING: usize = 43;
pub const WORD: usize = 44;
pub const CHAR: usize = 45;
pub const TOKEN_NAMES: &[&str] = &[
    "$end",
    "INVALID",
    "ER_DIAGRAM",
    "EOF",
    "NEWLINE",
    "COLON",
    "STYLE_SEPARATOR",
    "BLOCK_START",
    "BLOCK_STOP",
    "SQS",
    "SQE",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "END",
    "SUBGRAPH",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "CLASSDEF",
    "UNICODE_TEXT",
    "STYLE_TEXT",
    "COMMA",
    "CLASS",
    "STYLE",
    "SEMI",
    "NUM",
    "BRKT",
    "ENTITY_NAME",
    "DECIMAL_NUM",
    "ENTITY_ONE",
    "ATTRIBUTE_WORD",
    "ATTRIBUTE_KEY",
    "COMMENT",
    "ZERO_OR_ONE",
    "ZERO_OR_MORE",
    "ONE_OR_MORE",
    "ONLY_ONE",
    "MD_PARENT",
    "NON_IDENTIFYING",
    "IDENTIFYING",
    "WORD",
    "CHAR",
];
