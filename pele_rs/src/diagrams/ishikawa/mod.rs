mod render;
use crate::PeleError;
use crate::errors::syntax_error;
use crate::preprocess::Config;
use crate::util::{is_space16, is_word16, trim};
pub use render::render;
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub text: String,
    pub children: Vec<usize>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub nodes: Vec<Node>,
}
fn code(s: &[u16], p: usize) -> u16 {
    s.get(p).copied().unwrap_or(u16::MAX)
}
fn keyword(s: &[u16], at: usize) -> usize {
    let word = b"ishikawa-beta";
    let mut k = 0;
    while k < 13 {
        let c = code(s, at + k);
        let w = word[k] as u16;
        if c != w && (!(65..=90).contains(&c) || c + 32 != w) {
            break;
        }
        k += 1
    }
    if k == 13 && !is_word16(code(s, at + 13) as u32) {
        13
    } else if k >= 8 && !is_word16(code(s, at + 8) as u32) {
        8
    } else {
        0
    }
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let src: Vec<u16> = source.encode_utf16().collect();
    let n = src.len();
    let mut tokens: Vec<(usize, usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        let mut j = i;
        let mut last = None;
        while j < n && is_space16(src[j] as u32) {
            if src[j] == 10 {
                last = Some(j)
            }
            j += 1
        }
        let (kind, end) = if code(&src, j) == 37 && code(&src, j + 1) == 37 {
            let mut e = j + 2;
            while e < n && !matches!(src[e], 10 | 13 | 0x2028 | 0x2029) {
                e += 1
            }
            (2, e)
        } else if j == i {
            let len = keyword(&src, i);
            if len > 0 {
                (4, i + len)
            } else {
                (6, (i..n).find(|&p| src[p] == 10).unwrap_or(n))
            }
        } else if last.is_some_and(|p| p > i) {
            (2, last.unwrap() + 1)
        } else if last == Some(i) {
            (3, i + 1)
        } else {
            (5, j)
        };
        tokens.push((kind, i, end));
        i = end
    }
    tokens.push((1, n, n));
    tokens.push((0, n, n));
    let names = ["$end", "EOF", "SPACELINE", "NL", "ISHIKAWA", "SPACELIST", "TEXT"];
    let get = |i: usize| tokens.get(i).copied().unwrap_or((0, n, n));
    let fail = |i: usize, expected: &str| {
        syntax_error(
            "ishikawa",
            &src,
            get(i).1,
            &format!("Expecting {expected}, got '{}'", names[get(i).0]),
            false,
        )
    };
    i = 0;
    if get(i).0 == 2 {
        i += 1;
        while matches!(get(i).0, 2 | 3) {
            i += 1
        }
        if get(i).0 != 4 {
            return Err(fail(i, "'SPACELINE', 'NL', 'ISHIKAWA'"));
        }
    } else if get(i).0 != 4 {
        return Err(fail(i, "'SPACELINE', 'ISHIKAWA'"));
    }
    i += 1;
    if get(i).0 == 3 {
        i += 1
    }
    let mut model = Model {
        title: title.map(str::to_string),
        nodes: Vec::new(),
    };
    let mut stack: Vec<(usize, isize)> = Vec::new();
    let mut base = None;
    loop {
        let kind = get(i).0;
        if kind == 6 || (kind == 5 && get(i + 1).0 == 6) {
            let text = if kind == 6 { i } else { i + 1 };
            i = text + 1;
            if !matches!(get(i).0, 1..=3) {
                return Err(fail(i, "'SPACELINE', 'NL', 'EOF'"));
            }
            let raw_level = if kind == 6 { 0 } else { get(text - 1).2 - get(text - 1).1 } as isize;
            let text = trim(&String::from_utf16_lossy(&src[get(text).1..get(text).2])).to_string();
            let index = model.nodes.len();
            model.nodes.push(Node {
                text,
                children: Vec::new(),
            });
            if index == 0 {
                stack.push((0, 0))
            } else {
                let base = *base.get_or_insert(raw_level);
                let level = (raw_level - base + 1).max(1);
                while stack.len() > 1 && stack.last().unwrap().1 >= level {
                    stack.pop();
                }
                model.nodes[stack.last().unwrap().0].children.push(index);
                stack.push((index, level))
            }
        } else if kind == 2 || kind == 5 {
            i += 1;
            if !matches!(get(i).0, 1..=3) {
                return Err(fail(
                    i,
                    if kind == 5 {
                        "'SPACELINE', 'NL', 'EOF', 'TEXT'"
                    } else {
                        "'SPACELINE', 'NL', 'EOF'"
                    },
                ));
            }
        } else {
            return Err(fail(i, "'SPACELINE', 'SPACELIST', 'TEXT'"));
        }
        i += 1;
        while matches!(get(i).0, 3 | 1) {
            i += 1
        }
        if get(i).0 == 0 {
            return Ok(model);
        }
    }
}
