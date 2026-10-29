// The pie chart parser, after pele's diagrams/pie/parser.ts.

use crate::diagrams::tokens::{
    Pattern, Reader, TokenType, acc_descr_type, acc_descr_value, acc_title_type, acc_title_value, comment_type, directive_type, keyword,
    newline_type, string_type, string_value, title_type, title_value, tokenize, whitespace_type, yaml_type,
};
use crate::errors::PeleError;
use crate::util::js_number;

const SHOW_DATA: i32 = 0;
const PIE: i32 = 1;
const COLON: i32 = 2;
const NUMBER: i32 = 3;
const ACC_DESCR: i32 = 4;
const ACC_TITLE: i32 = 5;
const TITLE: i32 = 6;
const STRING: i32 = 7;
const NEWLINE: i32 = 8;

fn digits(src: &[u16], mut i: usize) -> usize {
    while src.get(i).is_some_and(|c| (48..=57).contains(c)) {
        i += 1;
    }
    i
}

fn code(src: &[u16], i: usize) -> u16 {
    src.get(i).copied().unwrap_or(u16::MAX)
}

// /(?:-?[0-9]+\.[0-9]+(?!\.))|(?:-?(0|[1-9][0-9]*)(?!\.))/y
fn number(src: &[u16], p: usize) -> Option<usize> {
    let at = if code(src, p) == 45 { p + 1 } else { p };
    let end = digits(src, at);
    if end == at {
        return None;
    }
    if code(src, end) == 46 {
        let stop = digits(src, end + 1);
        let length = stop - end - 1;
        if length > 0 && code(src, stop) != 46 {
            return Some(stop);
        }
        if length > 1 {
            return Some(stop - 1);
        }
    }
    if code(src, at) == 48 {
        return if code(src, at + 1) == 46 { None } else { Some(at + 1) };
    }
    if code(src, end) != 46 {
        return Some(end);
    }
    if end - 1 > at { Some(end - 1) } else { None }
}

pub fn pie_tokens() -> Vec<TokenType> {
    vec![
        keyword("showData"),
        keyword("pie"),
        TokenType {
            name: ":",
            pattern: Pattern::Literal(":"),
            hidden: false,
            first: None,
        },
        TokenType {
            name: "NUMBER_PIE",
            pattern: Pattern::Match(number),
            hidden: false,
            first: Some("-0123456789"),
        },
        acc_descr_type(),
        acc_title_type(),
        title_type(),
        string_type(),
        newline_type(),
        whitespace_type(),
        yaml_type(),
        directive_type(),
        comment_type(),
    ]
}

#[derive(Debug, Default)]
pub struct PieAst {
    pub show_data: bool,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub sections: Vec<(String, f64)>,
}

pub fn parse_pie(source: &str) -> Result<PieAst, PeleError> {
    let types = pie_tokens();
    let src: Vec<u16> = source.encode_utf16().collect();
    let tokens = tokenize(&src, &types, "pie")?;
    let mut r = Reader {
        i: 0,
        tokens,
        types: &types,
        diagram: "pie",
    };
    let mut ast = PieAst::default();

    let end_of_line = |r: &mut Reader| -> Result<(), PeleError> {
        if r.kind() == -1 {
            return Ok(());
        }
        if r.kind() != NEWLINE {
            return Err(r.fail("a line break or the end of input"));
        }
        while r.kind() == NEWLINE {
            r.i += 1;
        }
        Ok(())
    };

    while r.kind() == NEWLINE {
        r.i += 1;
    }
    r.expect(PIE)?;
    ast.show_data = r.accept(SHOW_DATA);
    while r.kind() != -1 {
        match r.kind() {
            NEWLINE => r.i += 1,
            ACC_DESCR => {
                ast.acc_descr = Some(acc_descr_value(&r.take()));
                end_of_line(&mut r)?;
            }
            ACC_TITLE => {
                ast.acc_title = Some(acc_title_value(&r.take()));
                end_of_line(&mut r)?;
            }
            TITLE => {
                ast.title = Some(title_value(&r.take()));
                end_of_line(&mut r)?;
            }
            STRING => {
                let label = string_value(&r.take());
                r.expect(COLON)?;
                let value = js_number(&r.expect(NUMBER)?);
                ast.sections.push((label, value));
                end_of_line(&mut r)?;
            }
            _ => return Err(r.fail("a section, a title, or a line break")),
        }
    }
    Ok(ast)
}
