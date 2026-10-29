use super::model::*;
use crate::{
    PeleError,
    diagrams::tokens::{self as t, Pattern, Reader, TokenType},
};
fn ascii_word(c: u16) -> bool {
    c == 95 || (c as u32 <= 127 && (c as u8).is_ascii_alphanumeric())
}
fn id(s: &[u16], p: usize) -> Option<usize> {
    if !s.get(p).is_some_and(|&c| ascii_word(c)) {
        return None;
    }
    let mut q = p + 1;
    while q < s.len() && (ascii_word(s[q]) || s[q] == 45) {
        q += 1;
    }
    while q > p + 1 && s[q - 1] == 45 {
        q -= 1;
    }
    Some(q)
}
fn word(s: &[u16], p: usize, w: &str) -> Option<usize> {
    let e = p + w.len();
    if s.get(p..e) != Some(w.encode_utf16().collect::<Vec<_>>().as_slice()) {
        return None;
    }
    if id(s, p).is_some_and(|q| q > e) { None } else { Some(e) }
}
macro_rules! words {($($f:ident:$w:literal),*)=>{$(fn $f(s:&[u16],p:usize)->Option<usize>{word(s,p,$w)})*}}
words!(header:"architecture-beta",junction:"junction",service:"service",column:"column",group:"group",align:"align",row:"row",inside:"in");
fn dash(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) == Some(&45) && t::yaml(s, p).is_none() {
        Some(p + 1)
    } else {
        None
    }
}
fn dashes(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p..p + 2) == Some(&[45, 45]) && t::yaml(s, p).is_none() {
        Some(p + 2)
    } else {
        None
    }
}
fn direction(s: &[u16], p: usize) -> Option<usize> {
    s.get(p).is_some_and(|&c| matches!(c, 76 | 82 | 84 | 66)).then_some(p + 1)
}
fn into(s: &[u16], p: usize) -> Option<usize> {
    s.get(p).is_some_and(|&c| matches!(c, 60 | 62)).then_some(p + 1)
}
fn title(s: &[u16], p: usize) -> Option<usize> {
    let e = t::title(s, p)?;
    let mut q = p;
    while s.get(q).is_some_and(|&c| c == 9 || c == 32) {
        q += 1;
    }
    if id(s, q).is_some_and(|n| n > q + 5) { None } else { Some(e) }
}
fn icon(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&40) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|&c| ascii_word(c) || c == 45 || c == 58) {
        q += 1;
    }
    if q > p + 1 && s.get(q) == Some(&41) { Some(q + 1) } else { None }
}
fn label(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&91) {
        return None;
    }
    if let Some(e) = t::string(s, p + 1)
        && s.get(e) == Some(&93)
    {
        return Some(e + 1);
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|&c| !matches!(c, 91 | 93 | 10 | 13)) {
        q += 1;
    }
    if q > p + 1 && s.get(q) == Some(&93) { Some(q + 1) } else { None }
}
fn token(name: &'static str, matcher: t::Matcher, hidden: bool) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(matcher),
        hidden,
        first: None,
    }
}
fn tokens() -> Vec<TokenType> {
    vec![
        token("architecture-beta", header, false),
        token("junction", junction, false),
        token("service", service, false),
        token("column", column, false),
        token("group", group, false),
        token("align", align, false),
        token("row", row, false),
        token("--", dashes, false),
        token("in", inside, false),
        TokenType {
            name: ":",
            pattern: Pattern::Literal(":"),
            hidden: false,
            first: None,
        },
        token("-", dash, false),
        token("ARROW_DIRECTION", direction, false),
        TokenType {
            name: "ARROW_GROUP",
            pattern: Pattern::Literal("{group}"),
            hidden: false,
            first: None,
        },
        token("ARROW_INTO", into, false),
        t::acc_descr_type(),
        t::acc_title_type(),
        token("TITLE", title, false),
        token("STRING", t::string, false),
        token("ID", id, false),
        token("NEWLINE", t::newline, false),
        token("WHITESPACE", t::whitespace, true),
        token("YAML", t::yaml, true),
        token("DIRECTIVE", t::directive, true),
        token("SINGLE_LINE_COMMENT", t::comment, true),
        token("ARCH_ICON", icon, false),
        token("ARCH_TITLE", label, false),
    ]
}
fn label_value(s: String) -> String {
    let s = s[1..s.len() - 1].trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].replace("\\\"", "\"").replace("\\'", "'").trim().into()
    } else {
        s.trim().into()
    }
}
fn side(s: String) -> usize {
    match s.as_str() {
        "L" => 0,
        "R" => 1,
        "T" => 2,
        _ => 3,
    }
}
pub fn parse(src: &str, model: &mut Model) -> Result<(), PeleError> {
    let types = tokens();
    let mut r = Reader {
        i: 0,
        tokens: t::tokenize(&src.encode_utf16().collect::<Vec<_>>(), &types, "architecture")?,
        types: &types,
        diagram: "architecture",
    };
    let (mut groups, mut services, mut junctions, mut edges, mut hints) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    while r.accept(19) {}
    r.expect(0)?;
    while r.kind() != -1 {
        match r.kind() {
            19 => {
                r.i += 1;
                continue;
            }
            14 => {
                let v = t::acc_descr_value(&r.take());
                if !v.is_empty() {
                    model.acc_descr = Some(v);
                }
            }
            15 => {
                let v = t::acc_title_value(&r.take());
                if !v.is_empty() {
                    model.acc_title = Some(v);
                }
            }
            16 => {
                let v = t::title_value(&r.take());
                if !v.is_empty() {
                    model.title = Some(v);
                }
            }
            4 => {
                r.i += 1;
                let mut g = Group {
                    id: r.expect(18)?,
                    ..Default::default()
                };
                if r.kind() == 24 {
                    let s = r.take();
                    g.icon = Some(s[1..s.len() - 1].into());
                }
                if r.kind() == 25 {
                    g.title = Some(label_value(r.take()));
                }
                if r.accept(8) {
                    g.parent = Some(r.expect(18)?);
                }
                groups.push(g);
            }
            2 => {
                r.i += 1;
                let mut n = Node {
                    id: r.expect(18)?,
                    ..Default::default()
                };
                if r.kind() == 17 {
                    n.icon_text = Some(t::string_value(&r.take()));
                } else if r.kind() == 24 {
                    let s = r.take();
                    n.icon = Some(s[1..s.len() - 1].into());
                }
                if r.kind() == 25 {
                    n.title = Some(label_value(r.take()));
                }
                if r.accept(8) {
                    n.parent = Some(r.expect(18)?);
                }
                services.push(n);
            }
            1 => {
                r.i += 1;
                let mut n = Node {
                    id: r.expect(18)?,
                    junction: true,
                    ..Default::default()
                };
                if r.accept(8) {
                    n.parent = Some(r.expect(18)?);
                }
                junctions.push(n);
            }
            18 => {
                let lhs_id = r.take();
                let lhs_group = r.accept(12);
                r.expect(9)?;
                let lhs_dir = side(r.expect(11)?);
                let lhs_into = r.accept(13);
                let title = if r.accept(7) {
                    None
                } else {
                    if !r.accept(10) {
                        return Err(r.fail("'--' or '-'"));
                    }
                    let title = label_value(r.expect(25)?);
                    r.expect(10)?;
                    Some(title)
                };
                let rhs_into = r.accept(13);
                let rhs_dir = side(r.expect(11)?);
                r.expect(9)?;
                let rhs_id = r.expect(18)?;
                let rhs_group = r.accept(12);
                edges.push(Edge {
                    lhs_id,
                    lhs_group,
                    lhs_dir,
                    lhs_into,
                    title,
                    rhs_into,
                    rhs_dir,
                    rhs_id,
                    rhs_group,
                });
            }
            5 => {
                r.i += 1;
                if r.kind() != 6 && r.kind() != 3 {
                    return Err(r.fail("'row' or 'column'"));
                }
                let direction = r.take();
                let mut members = vec![r.expect(18)?, r.expect(18)?];
                while r.kind() == 18 {
                    members.push(r.take());
                }
                hints.push(Hint { direction, members });
            }
            _ => return Err(r.fail("a group, a service, a junction, an edge, an alignment, a title, or a line break")),
        }
        if r.kind() != -1 {
            if r.kind() != 19 {
                return Err(r.fail("a line break or the end of input"));
            }
            while r.accept(19) {}
        }
    }
    for g in groups {
        model.add_group(g)?;
    }
    for n in services.into_iter().chain(junctions) {
        model.add_node(n)?;
    }
    for e in edges {
        model.add_edge(e)?;
    }
    for h in hints {
        model.add_hint(h)?;
    }
    Ok(())
}
