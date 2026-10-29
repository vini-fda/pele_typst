// Turns pele's SVG, which is themed with CSS custom properties, into SVG that needs no CSS: every
// var() is resolved, color-mix() and calc() are worked out, and style declarations become
// presentation attributes. Renderers without CSS support, such as the one in Typst, can draw it.

use std::collections::HashMap;

// Custom properties set by the host, such as `--pele-fg`, as if on an element around the diagram.
pub fn inline_styles(svg: &str, theme: &[(String, String)]) -> String {
    let mut scopes: Vec<HashMap<String, String>> = vec![theme.iter().cloned().collect()];
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        if rest.starts_with("</") {
            let end = rest.find('>').map_or(rest.len(), |i| i + 1);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            if scopes.len() > 1 {
                scopes.pop();
            }
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            let end = rest.find('>').map_or(rest.len(), |i| i + 1);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        let (tag, used) = parse_tag(rest);
        rest = &rest[used..];
        let mut scope: HashMap<String, String> = HashMap::new();
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut styles: Vec<(String, String)> = Vec::new();
        for (name, value) in tag.attrs {
            if name == "style" {
                for decl in split_top(&value, ';') {
                    if let Some(at) = decl.find(':') {
                        let prop = decl[..at].trim().to_string();
                        let val = decl[at + 1..].trim().to_string();
                        if prop.starts_with("--") {
                            let resolved = resolve(&val, &scopes, &scope);
                            scope.insert(prop, resolved);
                        } else if !prop.is_empty() {
                            styles.push((prop.to_ascii_lowercase(), val));
                        }
                    }
                }
            } else {
                attrs.push((name, value));
            }
        }
        for (_, value) in attrs.iter_mut() {
            if value.contains('(') {
                *value = resolve(value, &scopes, &scope);
            }
        }
        for (prop, value) in styles {
            let value = resolve(&value, &scopes, &scope);
            for (name, value) in presentation(&prop, &value) {
                match attrs.iter_mut().find(|(n, _)| *n == name) {
                    Some(slot) => slot.1 = value,
                    None => attrs.push((name, value)),
                }
            }
        }
        out.push('<');
        out.push_str(&tag.name);
        for (name, value) in &attrs {
            out.push_str(&format!(" {}=\"{}\"", name, value.replace('"', "&quot;")));
        }
        if tag.closed {
            out.push_str("/>");
        } else {
            out.push('>');
            scopes.push(scope);
        }
    }
    out.push_str(rest);
    out
}

struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
    closed: bool,
}

// Reads a start tag at the start of `s`, whose attribute values are quoted with ".
fn parse_tag(s: &str) -> (Tag, usize) {
    let b = s.as_bytes();
    let mut i = 1;
    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
        i += 1;
    }
    let name = s[1..i].to_string();
    let mut attrs = Vec::new();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return (
                Tag {
                    name,
                    attrs,
                    closed: false,
                },
                i,
            );
        }
        if b[i] == b'>' {
            return (
                Tag {
                    name,
                    attrs,
                    closed: false,
                },
                i + 1,
            );
        }
        if b[i] == b'/' && b.get(i + 1) == Some(&b'>') {
            return (Tag { name, attrs, closed: true }, i + 2);
        }
        let start = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() && b[i] != b'>' {
            i += 1;
        }
        let attr = s[start..i].to_string();
        if b.get(i) == Some(&b'=') && b.get(i + 1) == Some(&b'"') {
            let v0 = i + 2;
            let v1 = s[v0..].find('"').map_or(s.len(), |k| v0 + k);
            attrs.push((attr, s[v0..v1].to_string()));
            i = (v1 + 1).min(s.len());
        } else {
            attrs.push((attr, String::new()));
            if i == start {
                i += 1;
            }
        }
    }
}

fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if c == sep && depth <= 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

// The presentation attributes a CSS declaration stands for. Those that only matter in a page are dropped.
fn presentation(prop: &str, value: &str) -> Vec<(String, String)> {
    match prop {
        "max-width" | "height" | "width" | "text-transform" | "direction" | "cursor" | "pointer-events" => Vec::new(),
        "letter-spacing" | "word-spacing" if value == "normal" => Vec::new(),
        "font" => {
            let mut parts = value.splitn(2, char::is_whitespace);
            let size = parts.next().unwrap_or("");
            let family = parts.next().unwrap_or("").trim();
            let mut out = vec![("font-size".to_string(), size.trim_end_matches("px").to_string())];
            if !family.is_empty() {
                out.push(("font-family".to_string(), family.to_string()));
            }
            out
        }
        _ => vec![(prop.to_string(), value.to_string())],
    }
}

fn lookup(name: &str, scopes: &[HashMap<String, String>], own: &HashMap<String, String>) -> Option<String> {
    if let Some(v) = own.get(name) {
        return Some(v.clone());
    }
    scopes.iter().rev().find_map(|s| s.get(name).cloned())
}

// The text from `open` (just past a "(") to its matching ")", and where that ends.
fn group(s: &str, open: usize) -> Option<(String, usize)> {
    let mut depth = 1;
    for (i, c) in s[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((s[open..open + i].to_string(), open + i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn resolve(value: &str, scopes: &[HashMap<String, String>], own: &HashMap<String, String>) -> String {
    let mut v = value.to_string();
    // var() may nest in a fallback, so substitute until none is left.
    for _ in 0..32 {
        let Some(at) = v.find("var(") else { break };
        let Some((inner, end)) = group(&v, at + 4) else { break };
        let (name, fallback) = match split_top(&inner, ',').as_slice() {
            [] => (String::new(), None),
            [name] => (name.trim().to_string(), None),
            _ => {
                let comma = inner.find(',').unwrap();
                (inner[..comma].trim().to_string(), Some(inner[comma + 1..].trim().to_string()))
            }
        };
        let replacement = lookup(&name, scopes, own).or(fallback).unwrap_or_default();
        v = format!("{}{}{}", &v[..at], replacement, &v[end..]);
    }
    for _ in 0..8 {
        let Some(at) = v.find("color-mix(") else { break };
        let Some((inner, end)) = group(&v, at + 10) else { break };
        let mixed = color_mix(&inner).unwrap_or_default();
        v = format!("{}{}{}", &v[..at], mixed, &v[end..]);
    }
    for _ in 0..8 {
        let Some(at) = v.find("calc(") else { break };
        let Some((inner, end)) = group(&v, at + 5) else { break };
        let sum = calc(&inner).map(crate::util::num).unwrap_or_default();
        v = format!("{}{}{}", &v[..at], sum, &v[end..]);
    }
    v
}

// Sums of lengths in px, as in `2px + 4px`.
fn calc(expr: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut sign = 1.0;
    for token in expr.split_whitespace() {
        match token {
            "+" => sign = 1.0,
            "-" => sign = -1.0,
            t => {
                let n: f64 = t.trim_end_matches("px").parse().ok()?;
                total += sign * n;
            }
        }
    }
    Some(total)
}

// `in srgb, A p%, B`, with at most one percentage.
fn color_mix(inner: &str) -> Option<String> {
    let parts = split_top(inner, ',');
    if parts.len() != 3 {
        return None;
    }
    let one = |s: &str| -> Option<([f64; 4], Option<f64>)> {
        let s = s.trim();
        match s.rsplit_once(char::is_whitespace) {
            Some((color, pct)) if pct.ends_with('%') => Some((
                parse_color(color.trim())?,
                Some(pct.trim_end_matches('%').parse::<f64>().ok()? / 100.0),
            )),
            _ => Some((parse_color(s)?, None)),
        }
    };
    let (a, pa) = one(&parts[1])?;
    let (b, pb) = one(&parts[2])?;
    let wa = match (pa, pb) {
        (Some(p), _) => p,
        (None, Some(q)) => 1.0 - q,
        (None, None) => 0.5,
    };
    let mix = |i: usize| a[i] * wa + b[i] * (1.0 - wa);
    Some(format_color([mix(0), mix(1), mix(2), mix(3)]))
}

fn format_color(c: [f64; 4]) -> String {
    let byte = |v: f64| (v.clamp(0.0, 255.0)).round() as u8;
    if c[3] >= 0.999 {
        format!("#{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]))
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]), byte(c[3] * 255.0))
    }
}

// RGB in 0..255 and alpha in 0..1, from hex, rgb()/rgba() and a few names.
fn parse_color(s: &str) -> Option<[f64; 4]> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        let digits: String = match hex.len() {
            3 | 4 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => hex.to_string(),
            _ => return None,
        };
        let n = |i: usize| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).ok().map(f64::from);
        let alpha = if digits.len() == 8 { n(3)? / 255.0 } else { 1.0 };
        return Some([n(0)?, n(1)?, n(2)?, alpha]);
    }
    if let Some(body) = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))
        .and_then(|b| b.strip_suffix(')'))
    {
        let parts: Vec<&str> = body
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let channel = |p: &str| -> Option<f64> {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f64>().ok().map(|v| v * 2.55)
            } else {
                p.parse().ok()
            }
        };
        let alpha = match parts.get(3) {
            Some(p) => {
                if let Some(pct) = p.strip_suffix('%') {
                    pct.parse::<f64>().ok()? / 100.0
                } else {
                    p.parse().ok()?
                }
            }
            None => 1.0,
        };
        return Some([channel(parts[0])?, channel(parts[1])?, channel(parts[2])?, alpha]);
    }
    match s.as_str() {
        "white" => Some([255.0, 255.0, 255.0, 1.0]),
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "transparent" => Some([0.0, 0.0, 0.0, 0.0]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_variables_and_mixes() {
        let svg = r#"<svg style="--_bg:var(--pele-bg,#fff);--_a:var(--pele-surface-alt,#e6e6e6);color:var(--_fg,#123);font:16px sans-serif;max-width:100%" fill="var(--_bg)"><rect style="rx:var(--_r,4px)" stroke="color-mix(in srgb,var(--_a) 50%,var(--_bg))"/><g style="stroke-width:calc(2px + 4px)"></g></svg>"#;
        let out = inline_styles(svg, &[("--pele-bg".into(), "#000000".into())]);
        assert_eq!(
            out,
            r##"<svg fill="#000000" color="#123" font-size="16" font-family="sans-serif"><rect stroke="#737373" rx="4px"/><g stroke-width="6"></g></svg>"##
        );
    }
}
