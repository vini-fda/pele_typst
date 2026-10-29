use super::{Model, Node, Rule};
use crate::PeleError;
use crate::diagrams::timeline::story;
use crate::diagrams::tokens::{self, Pattern, Reader, TokenType};
use crate::errors::PeleErrorCode;
use crate::preprocess::Config;
use crate::util::{is_space16, is_word16, trim, trim_start};
fn starts(s: &[u16], p: usize, t: &str) -> bool {
    t.encode_utf16().enumerate().all(|(i, c)| s.get(p + i) == Some(&c))
}
fn id(s: &[u16], p: usize) -> Option<usize> {
    if !s.get(p).is_some_and(|c| matches!(c,65..=90|97..=122|95)) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|c| is_word16(*c as u32) || *c == 45) {
        q += 1
    }
    Some(q)
}
fn abnf_id(s: &[u16], p: usize) -> Option<usize> {
    if !s.get(p).is_some_and(|c| matches!(c,65..=90|97..=122)) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|c| matches!(c,65..=90|97..=122|48..=57|45)) {
        q += 1
    }
    Some(q)
}
fn title(s: &[u16], p: usize) -> Option<usize> {
    if !starts(s, p, "title") {
        return None;
    }
    if s.get(p + 5).is_some_and(|c| is_word16(*c as u32) || *c == 45) {
        return None;
    }
    tokens::title(s, p)
}
fn abnf_title(s: &[u16], p: usize) -> Option<usize> {
    if !starts(s, p, "title") {
        return None;
    }
    if s.get(p + 5).is_some_and(|c| matches!(c,65..=90|97..=122|48..=57|45)) {
        return None;
    }
    tokens::title(s, p)
}
fn acc_title(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) == Some(&97) { tokens::acc_title(s, p) } else { None }
}
fn acc_descr(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) == Some(&97) { tokens::acc_descr(s, p) } else { None }
}
fn ws(s: &[u16], p: usize) -> Option<usize> {
    let mut q = p;
    while s.get(q).is_some_and(|c| matches!(c, 9 | 10 | 13 | 32)) {
        q += 1
    }
    (q > p).then_some(q)
}
fn block(s: &[u16], p: usize) -> Option<usize> {
    if !starts(s, p, "/*") {
        return None;
    }
    (p + 2..s.len()).find(|&i| starts(s, i, "*/")).map(|i| i + 2)
}
fn iso(s: &[u16], p: usize) -> Option<usize> {
    if !starts(s, p, "(*") {
        return None;
    }
    (p + 2..s.len()).find(|&i| starts(s, i, "*)")).map(|i| i + 2)
}
fn special(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&63) {
        return None;
    }
    let mut solid = false;
    for i in p + 1..s.len() {
        if s[i] == 63 {
            return solid.then_some(i + 1);
        }
        if s[i] == 59 {
            return None;
        }
        solid |= !is_space16(s[i] as u32)
    }
    None
}
fn group(s: &[u16], p: usize) -> Option<usize> {
    (s.get(p) == Some(&40) && iso(s, p).is_none()).then_some(p + 1)
}
fn question(s: &[u16], p: usize) -> Option<usize> {
    (s.get(p) == Some(&63) && special(s, p).is_none()).then_some(p + 1)
}
fn minus(s: &[u16], p: usize) -> Option<usize> {
    (s.get(p) == Some(&45) && tokens::yaml(s, p).is_none()).then_some(p + 1)
}
fn semi(s: &[u16], p: usize) -> Option<usize> {
    (s.get(p) == Some(&59) && (s.get(p + 1).is_none() || s.get(p + 1).is_some_and(|c| matches!(c, 10 | 13)))).then_some(p + 1)
}
fn abnf_comment(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&59) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|c| !matches!(c, 10 | 13)) {
        q += 1
    }
    Some(q)
}
fn peg_comment(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&35) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|c| !matches!(c, 10 | 13)) {
        q += 1
    }
    Some(q)
}
fn abnf_string(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&34) {
        return None;
    }
    (p + 1..s.len()).find(|&i| s[i] == 34).map(|i| i + 1)
}
fn numval(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&37) || !s.get(p + 1).is_some_and(|c| matches!(c, 120 | 88 | 100 | 68 | 98 | 66)) {
        return None;
    }
    let hex = |i: usize| s.get(i).is_some_and(|c| matches!(c,48..=57|65..=70|97..=102));
    let mut q = p + 2;
    while hex(q) {
        q += 1
    }
    if q == p + 2 {
        return None;
    }
    loop {
        if s.get(q).is_some_and(|c| matches!(c, 45 | 46)) && hex(q + 1) {
            q += 2;
            while hex(q) {
                q += 1
            }
        } else {
            break;
        }
    }
    Some(q)
}
fn repeat(s: &[u16], p: usize) -> Option<usize> {
    let mut q = p;
    while s.get(q).is_some_and(|c| matches!(c, 48..=57)) {
        q += 1
    }
    if s.get(q) != Some(&42) {
        return None;
    }
    q += 1;
    while s.get(q).is_some_and(|c| matches!(c, 48..=57)) {
        q += 1
    }
    Some(q)
}
fn exact(s: &[u16], p: usize) -> Option<usize> {
    let mut q = p;
    while s.get(q).is_some_and(|c| matches!(c, 48..=57)) {
        q += 1
    }
    (q > p).then_some(q)
}
macro_rules! word {
    ($f:ident,$w:expr) => {
        fn $f(s: &[u16], p: usize) -> Option<usize> {
            (starts(s, p, $w) && id(s, p).unwrap_or(p) <= p + $w.len()).then_some(p + $w.len())
        }
    };
}
word!(nonterminal, "nonterminal");
word!(zero, "zeroOrMore");
word!(one, "oneOrMore");
word!(sequence, "sequence");
word!(optional, "optional");
word!(terminal, "terminal");
word!(sp, "special");
word!(choice, "choice");
fn token(name: &'static str, f: tokens::Matcher, hidden: bool) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(f),
        hidden,
        first: None,
    }
}
fn lit(s: &'static str) -> TokenType {
    TokenType {
        name: s,
        pattern: Pattern::Literal(s),
        hidden: false,
        first: None,
    }
}
fn types(notation: &str) -> Vec<TokenType> {
    let mut v = match notation {
        "ebnf" => vec![
            tokens::keyword("railroad-ebnf-beta"),
            lit("::="),
            lit("="),
            lit(";"),
            lit("|"),
            lit(","),
            token("(", group, false),
            lit(")"),
            lit("["),
            lit("]"),
            lit("{"),
            lit("}"),
            token("?", question, false),
            lit("*"),
            lit("+"),
            token("-", minus, false),
        ],
        "abnf" => vec![
            tokens::keyword("railroad-abnf-beta"),
            lit("="),
            token(";", semi, false),
            lit("/"),
            lit("("),
            lit(")"),
            lit("["),
            lit("]"),
        ],
        "peg" => vec![
            tokens::keyword("railroad-peg-beta"),
            lit("<-"),
            lit(";"),
            lit("/"),
            lit("&"),
            lit("!"),
            lit("?"),
            lit("*"),
            lit("+"),
            lit("("),
            lit(")"),
            lit("."),
        ],
        _ => vec![
            tokens::keyword("railroad-beta"),
            token("nonterminal", nonterminal, false),
            token("zeroOrMore", zero, false),
            token("oneOrMore", one, false),
            token("sequence", sequence, false),
            token("optional", optional, false),
            token("terminal", terminal, false),
            token("special", sp, false),
            token("choice", choice, false),
            lit("="),
            lit(";"),
            lit("("),
            lit(","),
            lit(")"),
        ],
    };
    v.extend([
        token("TITLE", if notation == "abnf" { abnf_title } else { title }, false),
        token("ACC_TITLE", acc_title, false),
        token("ACC_DESCR", acc_descr, false),
    ]);
    match notation {
        "abnf" => v.extend([
            token("ABNF_RULENAME", abnf_id, false),
            token("ABNF_STRING", abnf_string, false),
            token("ABNF_NUMVAL", numval, false),
            token("ABNF_REPEAT", repeat, false),
            token("ABNF_EXACT_REPEAT", exact, false),
        ]),
        "ebnf" => v.extend([
            token("EBNF_ID", id, false),
            token("EBNF_STRING", tokens::string, false),
            token("EBNF_SPECIAL_SEQUENCE", special, false),
        ]),
        "peg" => v.extend([token("PEG_ID", id, false), token("PEG_STRING", tokens::string, false)]),
        _ => v.extend([token("RR_ID", id, false), token("RR_STRING", tokens::string, false)]),
    };
    v.extend([
        token("WHITESPACE", ws, true),
        token("YAML", tokens::yaml, true),
        token("DIRECTIVE", tokens::directive, true),
        token("SINGLE_LINE_COMMENT", tokens::comment, true),
    ]);
    match notation {
        "ebnf" => v.extend([token("EBNF_BLOCK_COMMENT", block, true), token("EBNF_ISO_COMMENT", iso, true)]),
        "abnf" => v.push(token("ABNF_COMMENT", abnf_comment, true)),
        "peg" => v.push(token("PEG_LINE_COMMENT", peg_comment, true)),
        _ => v.push(token("RR_BLOCK_COMMENT", block, true)),
    }
    v
}
fn kind(r: &Reader) -> &'static str {
    if r.kind() == -1 { "" } else { r.types[r.kind() as usize].name }
}
fn expect(r: &mut Reader, n: &str) -> Result<String, PeleError> {
    r.expect(r.types.iter().position(|t| t.name == n).unwrap() as i32)
}
fn accept(r: &mut Reader, n: &str) -> bool {
    if kind(r) == n {
        r.i += 1;
        true
    } else {
        false
    }
}
fn unescape(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 1;
    while i + 1 < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 2 < chars.len() {
            i += 1;
            out.push(match chars[i] {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                c => c,
            })
        } else {
            out.push(c)
        }
        i += 1
    }
    out
}
fn ir(r: &mut Reader, m: &mut Model) -> Result<usize, PeleError> {
    let mut stack: Vec<(&str, Vec<usize>)> = Vec::new();
    loop {
        let k = kind(r);
        let mut value = match k {
            "terminal" | "nonterminal" | "special" => {
                r.i += 1;
                expect(r, "(")?;
                let raw = unescape(&expect(r, "RR_STRING")?);
                expect(r, ")")?;
                m.add(match k {
                    "terminal" => Node::Terminal(raw),
                    "nonterminal" => Node::Nonterminal(raw),
                    _ => Node::Special(raw),
                })
            }
            "sequence" | "choice" | "optional" | "oneOrMore" | "zeroOrMore" => {
                r.i += 1;
                expect(r, "(")?;
                stack.push((k, Vec::new()));
                continue;
            }
            _ => return Err(r.fail("an expression")),
        };
        loop {
            let Some((k, kids)) = stack.last_mut() else { return Ok(value) };
            kids.push(value);
            if matches!(*k, "sequence" | "choice") && accept(r, ",") {
                break;
            }
            expect(r, ")")?;
            let (k, kids) = stack.pop().unwrap();
            value = match k {
                "sequence" => m.sequence(kids),
                "choice" => m.choice(kids),
                "optional" => m.add(Node::Optional(kids[0])),
                _ => m.repeat(kids[0], if k == "oneOrMore" { 1.0 } else { 0.0 }, f64::INFINITY),
            }
        }
    }
}
struct Frame {
    alts: Vec<usize>,
    seq: Vec<usize>,
    close: &'static str,
    wrapper: &'static str,
    prefix: Option<String>,
    except: bool,
}
impl Frame {
    fn new(close: &'static str, wrapper: &'static str, prefix: Option<String>, except: bool) -> Self {
        Self {
            alts: Vec::new(),
            seq: Vec::new(),
            close,
            wrapper,
            prefix,
            except,
        }
    }
    fn finish(mut self, m: &mut Model) -> usize {
        let seq = m.sequence(std::mem::take(&mut self.seq));
        self.alts.push(seq);
        m.choice(self.alts)
    }
}
fn ebnf(r: &mut Reader, m: &mut Model) -> Result<usize, PeleError> {
    let mut stack = Vec::new();
    let mut top = Frame::new("<root>", "", None, false);
    let mut except = false;
    loop {
        let k = kind(r);
        let mut primary = match k {
            "EBNF_STRING" => m.add(Node::Terminal(unescape(&r.take()))),
            "EBNF_ID" => m.add(Node::Nonterminal(r.take())),
            "EBNF_SPECIAL_SEQUENCE" => {
                let raw = r.take();
                m.add(Node::Special(trim(&raw[1..raw.len() - 1]).into()))
            }
            "(" | "[" | "{" => {
                r.i += 1;
                stack.push(top);
                top = Frame::new(
                    match k {
                        "(" => ")",
                        "[" => "]",
                        _ => "}",
                    },
                    k,
                    None,
                    except,
                );
                except = false;
                continue;
            }
            _ => return Err(r.fail("a terminal, a name, or an opening bracket")),
        };
        loop {
            if except {
                let last = top.seq.pop().unwrap();
                let minus = m.add(Node::Terminal("-".into()));
                primary = m.add(Node::Sequence(vec![last, minus, primary]));
                except = false
            }
            loop {
                match kind(r) {
                    "?" => {
                        r.i += 1;
                        primary = m.add(Node::Optional(primary))
                    }
                    "*" | "+" => {
                        let min = if kind(r) == "+" { 1.0 } else { 0.0 };
                        r.i += 1;
                        primary = m.repeat(primary, min, f64::INFINITY)
                    }
                    _ => break,
                }
            }
            top.seq.push(primary);
            let k = kind(r);
            if k == top.close {
                r.i += 1;
                let wrapper = top.wrapper;
                except = top.except;
                primary = top.finish(m);
                primary = match wrapper {
                    "[" => m.add(Node::Optional(primary)),
                    "{" => m.repeat(primary, 0.0, f64::INFINITY),
                    _ => primary,
                };
                top = stack.pop().unwrap();
                continue;
            }
            if k == "-" {
                r.i += 1;
                except = true
            } else if k == "," {
                r.i += 1
            } else if k == "|" {
                r.i += 1;
                let seq = m.sequence(std::mem::take(&mut top.seq));
                top.alts.push(seq)
            } else if !matches!(k, "EBNF_STRING" | "EBNF_ID" | "EBNF_SPECIAL_SEQUENCE" | "(" | "[" | "{") {
                if stack.is_empty() {
                    return Ok(top.finish(m));
                }
                return Err(r.fail(&format!("token of type '{}'", top.close)));
            }
            break;
        }
    }
}
fn apply_repeat(m: &mut Model, kid: usize, raw: Option<String>) -> usize {
    let Some(raw) = raw else { return kid };
    let (min, max) = if let Some(star) = raw.find('*') {
        (raw[..star].parse().unwrap_or(0.0), raw[star + 1..].parse().unwrap_or(f64::INFINITY))
    } else {
        let n = raw.parse().unwrap_or(f64::INFINITY);
        (n, n)
    };
    if min == 0.0 && max == 1.0 {
        m.add(Node::Optional(kid))
    } else {
        m.repeat(kid, min, max)
    }
}
fn abnf(r: &mut Reader, m: &mut Model) -> Result<usize, PeleError> {
    let mut stack = Vec::new();
    let mut top = Frame::new("<root>", "", None, false);
    loop {
        let mut repeat = if matches!(kind(r), "ABNF_REPEAT" | "ABNF_EXACT_REPEAT") {
            Some(r.take())
        } else {
            None
        };
        let k = kind(r);
        let mut primary = match k {
            "ABNF_STRING" => {
                let raw = r.take();
                m.add(Node::Terminal(raw[1..raw.len() - 1].into()))
            }
            "ABNF_NUMVAL" => m.add(Node::Terminal(r.take())),
            "ABNF_RULENAME" => m.add(Node::Nonterminal(r.take())),
            "(" | "[" => {
                r.i += 1;
                stack.push(top);
                top = Frame::new(if k == "(" { ")" } else { "]" }, k, repeat, false);
                continue;
            }
            _ => return Err(r.fail("a string, a number, a name, or an opening bracket")),
        };
        loop {
            top.seq.push(apply_repeat(m, primary, repeat));
            let k = kind(r);
            if k == top.close {
                r.i += 1;
                let wrapper = top.wrapper;
                repeat = top.prefix.take();
                primary = top.finish(m);
                if wrapper == "[" {
                    primary = m.add(Node::Optional(primary))
                }
                top = stack.pop().unwrap();
                continue;
            }
            if k == "/" {
                r.i += 1;
                let seq = m.sequence(std::mem::take(&mut top.seq));
                top.alts.push(seq)
            } else if !matches!(
                k,
                "ABNF_STRING" | "ABNF_NUMVAL" | "ABNF_RULENAME" | "(" | "[" | "ABNF_REPEAT" | "ABNF_EXACT_REPEAT"
            ) {
                if stack.is_empty() {
                    return Ok(top.finish(m));
                }
                return Err(r.fail(&format!("token of type '{}'", top.close)));
            }
            break;
        }
    }
}
fn peg_label(node: &Node) -> String {
    match node {
        Node::Terminal(v) => format!("\"{v}\""),
        Node::Nonterminal(v) | Node::Special(v) => v.clone(),
        _ => "(...)".into(),
    }
}
fn peg(r: &mut Reader, m: &mut Model) -> Result<usize, PeleError> {
    let mut stack = Vec::new();
    let mut top = Frame::new("<root>", "", None, false);
    loop {
        let mut op = if matches!(kind(r), "&" | "!") { Some(r.take()) } else { None };
        let mut primary = match kind(r) {
            "PEG_STRING" => m.add(Node::Terminal(unescape(&r.take()))),
            "PEG_ID" => m.add(Node::Nonterminal(r.take())),
            "." => m.add(Node::Special(r.take())),
            "(" => {
                r.i += 1;
                stack.push(top);
                top = Frame::new(")", "(", op, false);
                continue;
            }
            _ => return Err(r.fail("a literal, a name, a dot, or an opening parenthesis")),
        };
        loop {
            match kind(r) {
                "?" => {
                    r.i += 1;
                    primary = m.add(Node::Optional(primary))
                }
                "*" | "+" => {
                    let min = if kind(r) == "+" { 1.0 } else { 0.0 };
                    r.i += 1;
                    primary = m.repeat(primary, min, f64::INFINITY)
                }
                _ => (),
            }
            if let Some(op) = op {
                primary = m.add(Node::Special(op + &peg_label(&m.nodes[primary])))
            }
            top.seq.push(primary);
            let k = kind(r);
            if k == ")" && !stack.is_empty() {
                r.i += 1;
                op = top.prefix.take();
                primary = top.finish(m);
                top = stack.pop().unwrap();
                continue;
            }
            if k == "/" {
                r.i += 1;
                let seq = m.sequence(std::mem::take(&mut top.seq));
                top.alts.push(seq)
            } else if !matches!(k, "&" | "!" | "PEG_STRING" | "PEG_ID" | "." | "(") {
                if stack.is_empty() {
                    return Ok(top.finish(m));
                }
                return Err(r.fail("token of type ')'"));
            }
            break;
        }
    }
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let lower = trim_start(source).to_ascii_lowercase();
    let notation = if lower.starts_with("railroad-ebnf-beta") {
        "ebnf"
    } else if lower.starts_with("railroad-abnf-beta") {
        "abnf"
    } else if lower.starts_with("railroad-peg-beta") {
        "peg"
    } else if lower.starts_with("railroad-beta") {
        "railroad"
    } else {
        return Err(PeleError::new("No railroad diagram keyword found.", PeleErrorCode::Syntax).of("railroad"));
    };
    let ts = types(notation);
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut r = Reader {
        i: 0,
        tokens: tokens::tokenize(&src, &ts, "railroad")?,
        types: &ts,
        diagram: "railroad",
    };
    let mut m = Model {
        notation: notation.into(),
        title: title.filter(|s| !s.is_empty()).map(str::to_string),
        ..Default::default()
    };
    r.expect(0)?;
    loop {
        match kind(&r) {
            "TITLE" => {
                let mut v = tokens::title_value(&r.take());
                if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
                    v = if notation == "abnf" {
                        v[1..v.len() - 1].into()
                    } else {
                        unescape(&v)
                    }
                }
                m.title = if v.is_empty() {
                    title.filter(|s| !s.is_empty()).map(str::to_string)
                } else {
                    Some(v)
                };
            }
            "ACC_TITLE" => {
                let v = tokens::acc_title_value(&r.take());
                m.acc_title = (!v.is_empty()).then(|| story::acc(&v));
            }
            "ACC_DESCR" => {
                let v = tokens::acc_descr_value(&r.take());
                m.acc_descr = (!v.is_empty()).then(|| story::squeeze(&v));
            }
            _ => break,
        }
    }
    while r.kind() != -1 {
        let name = expect(
            &mut r,
            match notation {
                "ebnf" => "EBNF_ID",
                "abnf" => "ABNF_RULENAME",
                "peg" => "PEG_ID",
                _ => "RR_ID",
            },
        )?;
        if notation == "ebnf" {
            if !accept(&mut r, "=") {
                expect(&mut r, "::=")?;
            }
        } else {
            expect(&mut r, if notation == "peg" { "<-" } else { "=" })?;
        }
        let definition = match notation {
            "ebnf" => ebnf(&mut r, &mut m)?,
            "abnf" => abnf(&mut r, &mut m)?,
            "peg" => peg(&mut r, &mut m)?,
            _ => ir(&mut r, &mut m)?,
        };
        expect(&mut r, ";")?;
        m.rules.push(Rule { name, definition });
    }
    Ok(m)
}
