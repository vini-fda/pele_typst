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
            let terminator = if rest.starts_with("<!--") {
                "-->"
            } else if rest.starts_with("<![CDATA[") {
                "]]>"
            } else if rest.starts_with("<?") {
                "?>"
            } else {
                ">"
            };
            let end = rest.find(terminator).map_or(rest.len(), |i| i + terminator.len());
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
                            scope.insert(prop, val);
                        } else if !prop.is_empty() {
                            styles.push((prop.to_ascii_lowercase(), val));
                        }
                    }
                }
            } else {
                attrs.push((name, value));
            }
        }
        // Resolve after collecting declarations: CSS variables can refer forward to
        // another declaration on the same element. Descendants inherit the computed
        // value rather than re-evaluating it in their own scope.
        let mut scope: HashMap<String, String> = scope.iter().map(|(k, v)| (k.clone(), resolve_vars(v, &scopes, &scope))).collect();
        let inherited_color = lookup("@color", &scopes, &HashMap::new()).unwrap_or_else(|| "#000000".into());
        // color computes before paint values, even when its declaration appears
        // after them; currentColor in color itself refers to the inherited value.
        let declared_color = styles
            .iter()
            .rev()
            .find(|(name, _)| name == "color")
            .or_else(|| attrs.iter().rev().find(|(name, _)| name == "color"));
        let color = match declared_color.map(|(_, value)| resolve(value, &scopes, &scope)) {
            Some(value) if value.eq_ignore_ascii_case("initial") => "#000000".into(),
            Some(value)
                if !value.is_empty()
                    && !["currentcolor", "inherit", "unset", "revert", "revert-layer"]
                        .iter()
                        .any(|keyword| value.eq_ignore_ascii_case(keyword)) =>
            {
                value
            }
            _ => inherited_color,
        };
        scope.insert("@color".into(), color.clone());
        for (_, value) in attrs.iter_mut() {
            *value = resolve(value, &scopes, &scope);
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
        for (name, value) in &mut attrs {
            if name == "color"
                || matches!(name.as_str(), "fill" | "stroke" | "stop-color" | "flood-color" | "lighting-color")
                    && value.eq_ignore_ascii_case("currentcolor")
            {
                *value = color.clone();
            }
        }
        out.push('<');
        out.push_str(&tag.name);
        for (name, value) in &attrs {
            out.push_str(&format!(
                " {}=\"{}\"",
                name,
                value.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
            ));
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

// SVG assets may use either quote style, whitespace around '=', or nested SVG.
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
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if b.get(i) == Some(&b'=') {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if matches!(b.get(i), Some(b'"' | b'\'')) {
                let quote = b[i];
                let v0 = i + 1;
                let v1 = b[v0..].iter().position(|&c| c == quote).map_or(s.len(), |k| v0 + k);
                attrs.push((attr, unescape_attr(&s[v0..v1])));
                i = (v1 + 1).min(s.len());
            } else {
                let v0 = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                attrs.push((attr, unescape_attr(&s[v0..i])));
            }
        } else {
            attrs.push((attr, String::new()));
            if i == start {
                i += 1;
            }
        }
    }
}

fn unescape_attr(value: &str) -> String {
    // XML's five named entities and numeric character references are the only
    // entities permitted in SVG attributes without a document type declaration.
    let mut result = String::new();
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        result.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else { break };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            _ => entity
                .strip_prefix("#x")
                .and_then(|s| u32::from_str_radix(s, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|s| s.parse().ok()))
                .and_then(char::from_u32),
        };
        if let Some(c) = decoded {
            result.push(c);
            rest = &rest[end + 1..];
        } else {
            result.push('&');
            rest = &rest[1..];
        }
    }
    result.push_str(rest);
    result
}

fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote = None;
    let mut escaped = false;
    let mut cur = String::new();
    for c in s.chars() {
        if let Some(q) = quote {
            cur.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            cur.push(c);
            continue;
        }
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

fn resolve_vars(value: &str, scopes: &[HashMap<String, String>], own: &HashMap<String, String>) -> String {
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
    v
}

fn resolve(value: &str, scopes: &[HashMap<String, String>], own: &HashMap<String, String>) -> String {
    let mut v = resolve_vars(value, scopes, own);
    for _ in 0..32 {
        let Some(at) = v.find("color-mix(") else { break };
        let Some((inner, end)) = group(&v, at + 10) else { break };
        let current_color = lookup("@color", scopes, own).unwrap_or_else(|| "#000000".into());
        let Some(mixed) = color_mix(&inner, &current_color) else { break };
        v = format!("{}{}{}", &v[..at], mixed, &v[end..]);
    }
    for _ in 0..8 {
        let Some(at) = v.find("calc(") else { break };
        let Some((inner, end)) = group(&v, at + 5) else { break };
        let Some(sum) = calc(&inner).map(crate::util::num) else { break };
        v = format!("{}{}{}", &v[..at], sum, &v[end..]);
    }
    v
}

// Generated styles add casing widths with calc(). Accept ordinary arithmetic too,
// since the original stroke width may itself be a user-supplied calculation.
fn calc(expr: &str) -> Option<f64> {
    struct Parser<'a> {
        text: &'a [u8],
        at: usize,
        depth: usize,
    }
    impl Parser<'_> {
        fn space(&mut self) {
            while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
                self.at += 1;
            }
        }
        fn expr(&mut self) -> Option<f64> {
            let mut value = self.term()?;
            loop {
                self.space();
                match self.text.get(self.at) {
                    Some(b'+') => {
                        self.at += 1;
                        value += self.term()?;
                    }
                    Some(b'-') => {
                        self.at += 1;
                        value -= self.term()?;
                    }
                    _ => return Some(value),
                }
            }
        }
        fn term(&mut self) -> Option<f64> {
            let mut value = self.atom()?;
            loop {
                self.space();
                match self.text.get(self.at) {
                    Some(b'*') => {
                        self.at += 1;
                        value *= self.atom()?;
                    }
                    Some(b'/') => {
                        self.at += 1;
                        value /= self.atom()?;
                    }
                    _ => return Some(value),
                }
            }
        }
        fn atom(&mut self) -> Option<f64> {
            self.space();
            if self.depth >= 64 {
                return None;
            }
            self.depth += 1;
            let result = match self.text.get(self.at) {
                Some(b'+') => {
                    self.at += 1;
                    self.atom()
                }
                Some(b'-') => {
                    self.at += 1;
                    self.atom().map(|n| -n)
                }
                Some(b'(' | b'c') => {
                    if self.text.get(self.at) == Some(&b'c') {
                        if self.text.get(self.at..self.at + 5) != Some(b"calc(") {
                            return None;
                        }
                        self.at += 4;
                    }
                    self.at += 1;
                    let value = self.expr()?;
                    self.space();
                    if self.text.get(self.at) != Some(&b')') {
                        return None;
                    }
                    self.at += 1;
                    Some(value)
                }
                _ => {
                    let start = self.at;
                    while self.text.get(self.at).is_some_and(|c| c.is_ascii_digit() || *c == b'.') {
                        self.at += 1;
                    }
                    if self.at == start {
                        return None;
                    }
                    if matches!(self.text.get(self.at), Some(b'e' | b'E')) {
                        self.at += 1;
                        if matches!(self.text.get(self.at), Some(b'+' | b'-')) {
                            self.at += 1;
                        }
                        while self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
                            self.at += 1;
                        }
                    }
                    let value = std::str::from_utf8(&self.text[start..self.at]).ok()?.parse().ok()?;
                    if self.text.get(self.at..self.at + 2) == Some(b"px") {
                        self.at += 2;
                    }
                    Some(value)
                }
            };
            self.depth -= 1;
            result
        }
    }
    let mut parser = Parser {
        text: expr.as_bytes(),
        at: 0,
        depth: 0,
    };
    let value = parser.expr()?;
    parser.space();
    (parser.at == parser.text.len() && value.is_finite()).then_some(value)
}

// CSS mixes normalized weights and premultiplied components; transparent colors
// therefore do not darken the other color. A total below 100% reduces alpha.
fn color_mix(inner: &str, current_color: &str) -> Option<String> {
    mix_colors(inner, current_color, 0).map(format_color)
}

fn mix_colors(inner: &str, current_color: &str, depth: usize) -> Option<[f64; 4]> {
    if depth >= 64 {
        return None;
    }
    let parts = split_top(inner, ',');
    if parts.len() != 3 || !parts[0].trim().eq_ignore_ascii_case("in srgb") {
        return None;
    }
    let one = |s: &str| -> Option<([f64; 4], Option<f64>)> {
        let s = s.trim();
        let (color, weight) = match s.rsplit_once(char::is_whitespace) {
            Some((color, pct)) if pct.ends_with('%') => {
                let weight = pct.strip_suffix('%')?.parse::<f64>().ok()? / 100.;
                if !(0. ..=1.).contains(&weight) {
                    return None;
                }
                (color.trim(), Some(weight))
            }
            _ => (s, None),
        };
        let color = if color.eq_ignore_ascii_case("currentcolor") {
            current_color
        } else {
            color
        };
        let channels = if let Some(inner) = color.strip_prefix("color-mix(").and_then(|s| s.strip_suffix(')')) {
            mix_colors(inner, current_color, depth + 1)?
        } else {
            parse_color(color)?
        };
        Some((channels, weight))
    };
    let (a, pa) = one(&parts[1])?;
    let (b, pb) = one(&parts[2])?;
    let (wa, wb) = match (pa, pb) {
        (Some(p), Some(q)) => (p, q),
        (Some(p), None) => (p, 1. - p),
        (None, Some(q)) => (1. - q, q),
        (None, None) => (0.5, 0.5),
    };
    let sum = wa + wb;
    if sum == 0. {
        return None;
    }
    let (wa, wb) = (wa / sum, wb / sum);
    let alpha = a[3] * wa + b[3] * wb;
    let mix = |i: usize| {
        if alpha == 0. {
            0.
        } else {
            (a[i] * a[3] * wa + b[i] * b[3] * wb) / alpha
        }
    };
    Some([mix(0), mix(1), mix(2), alpha * sum.min(1.)])
}

fn format_color(c: [f64; 4]) -> String {
    let byte = |v: f64| v.clamp(0., 255.).round() as u8;
    let alpha = byte(c[3] * 255.);
    if alpha == 255 {
        format!("#{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]))
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2]), alpha)
    }
}

// CSS named colors are fixed sRGB values. Aliases retain the same value.
const NAMED_COLORS: &str = concat!(
    "aliceblue:f0f8ff antiquewhite:faebd7 aqua:00ffff aquamarine:7fffd4 azure:f0ffff beige:f5f5dc bisque:ffe4c4 black:000000 blanchedalmond:ffebcd blue:0000ff blueviolet:8a2be2 brown:a52a2a burlywood:deb887 ",
    "cadetblue:5f9ea0 chartreuse:7fff00 chocolate:d2691e coral:ff7f50 cornflowerblue:6495ed cornsilk:fff8dc crimson:dc143c cyan:00ffff darkblue:00008b darkcyan:008b8b darkgoldenrod:b8860b darkgray:a9a9a9 darkgreen:006400 darkgrey:a9a9a9 ",
    "darkkhaki:bdb76b darkmagenta:8b008b darkolivegreen:556b2f darkorange:ff8c00 darkorchid:9932cc darkred:8b0000 darksalmon:e9967a darkseagreen:8fbc8f darkslateblue:483d8b darkslategray:2f4f4f darkslategrey:2f4f4f darkturquoise:00ced1 darkviolet:9400d3 ",
    "deeppink:ff1493 deepskyblue:00bfff dimgray:696969 dimgrey:696969 dodgerblue:1e90ff firebrick:b22222 floralwhite:fffaf0 forestgreen:228b22 fuchsia:ff00ff gainsboro:dcdcdc ghostwhite:f8f8ff gold:ffd700 goldenrod:daa520 gray:808080 green:008000 greenyellow:adff2f grey:808080 ",
    "honeydew:f0fff0 hotpink:ff69b4 indianred:cd5c5c indigo:4b0082 ivory:fffff0 khaki:f0e68c lavender:e6e6fa lavenderblush:fff0f5 lawngreen:7cfc00 lemonchiffon:fffacd lightblue:add8e6 lightcoral:f08080 lightcyan:e0ffff lightgoldenrodyellow:fafad2 lightgray:d3d3d3 lightgreen:90ee90 lightgrey:d3d3d3 ",
    "lightpink:ffb6c1 lightsalmon:ffa07a lightseagreen:20b2aa lightskyblue:87cefa lightslategray:778899 lightslategrey:778899 lightsteelblue:b0c4de lightyellow:ffffe0 lime:00ff00 limegreen:32cd32 linen:faf0e6 magenta:ff00ff maroon:800000 mediumaquamarine:66cdaa mediumblue:0000cd mediumorchid:ba55d3 mediumpurple:9370db ",
    "mediumseagreen:3cb371 mediumslateblue:7b68ee mediumspringgreen:00fa9a mediumturquoise:48d1cc mediumvioletred:c71585 midnightblue:191970 mintcream:f5fffa mistyrose:ffe4e1 moccasin:ffe4b5 navajowhite:ffdead navy:000080 oldlace:fdf5e6 olive:808000 olivedrab:6b8e23 orange:ffa500 orangered:ff4500 orchid:da70d6 ",
    "palegoldenrod:eee8aa palegreen:98fb98 paleturquoise:afeeee palevioletred:db7093 papayawhip:ffefd5 peachpuff:ffdab9 peru:cd853f pink:ffc0cb plum:dda0dd powderblue:b0e0e6 purple:800080 rebeccapurple:663399 red:ff0000 rosybrown:bc8f8f royalblue:4169e1 saddlebrown:8b4513 salmon:fa8072 sandybrown:f4a460 seagreen:2e8b57 seashell:fff5ee sienna:a0522d silver:c0c0c0 ",
    "skyblue:87ceeb slateblue:6a5acd slategray:708090 slategrey:708090 snow:fffafa springgreen:00ff7f steelblue:4682b4 tan:d2b48c teal:008080 thistle:d8bfd8 tomato:ff6347 turquoise:40e0d0 violet:ee82ee wheat:f5deb3 white:ffffff whitesmoke:f5f5f5 yellow:ffff00 yellowgreen:9acd32"
);

fn finite(s: &str) -> Option<f64> {
    let n: f64 = s.parse().ok()?;
    n.is_finite().then_some(n)
}
fn alpha(s: &str) -> Option<f64> {
    if s == "none" {
        return Some(0.);
    }
    Some(
        if let Some(pct) = s.strip_suffix('%') {
            finite(pct)? / 100.
        } else {
            finite(s)?
        }
        .clamp(0., 1.),
    )
}
fn parse_color(s: &str) -> Option<[f64; 4]> {
    let s = s.trim().to_ascii_lowercase();
    if s == "transparent" {
        return Some([0.; 4]);
    }
    if let Some(hex) = s.strip_prefix('#') {
        // Validate bytes before slicing: malformed Unicode hex colors must not panic.
        if !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let digits: String = match hex.len() {
            3 | 4 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => hex.to_string(),
            _ => return None,
        };
        let n = |i: usize| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).ok().map(f64::from);
        return Some([n(0)?, n(1)?, n(2)?, if digits.len() == 8 { n(3)? / 255. } else { 1. }]);
    }
    let function = s
        .split_once('(')
        .and_then(|(name, body)| body.strip_suffix(')').map(|body| (name, body)));
    if let Some((name @ ("rgb" | "rgba" | "hsl" | "hsla"), body)) = function {
        let (parts, a) = if body.contains(',') {
            if body.contains('/') {
                return None;
            }
            let mut parts: Vec<_> = body.split(',').map(str::trim).collect();
            let a = if parts.len() == 4 { alpha(parts.pop()?)? } else { 1. };
            (parts, a)
        } else {
            let mut slash = body.split('/');
            let parts = slash.next()?.split_whitespace().collect::<Vec<_>>();
            let a = if let Some(a) = slash.next() { alpha(a.trim())? } else { 1. };
            if slash.next().is_some() {
                return None;
            }
            (parts, a)
        };
        if parts.len() != 3 {
            return None;
        }
        if name.starts_with("rgb") {
            let channel = |p: &str| -> Option<f64> {
                if p == "none" {
                    return Some(0.);
                }
                Some(
                    if let Some(pct) = p.strip_suffix('%') {
                        finite(pct)? * 255. / 100.
                    } else {
                        finite(p)?
                    }
                    .clamp(0., 255.),
                )
            };
            return Some([channel(parts[0])?, channel(parts[1])?, channel(parts[2])?, a]);
        }
        let hue = parts[0];
        let h = if hue == "none" {
            0.
        } else if let Some(v) = hue.strip_suffix("deg") {
            finite(v)?
        } else if let Some(v) = hue.strip_suffix("grad") {
            finite(v)? * 0.9
        } else if let Some(v) = hue.strip_suffix("rad") {
            finite(v)? * 180. / std::f64::consts::PI
        } else if let Some(v) = hue.strip_suffix("turn") {
            finite(v)? * 360.
        } else {
            finite(hue)?
        }
        .rem_euclid(360.)
            / 30.;
        let percent = |p: &str| {
            if p == "none" {
                Some(0.)
            } else {
                Some((finite(p.strip_suffix('%')?)? / 100.).clamp(0., 1.))
            }
        };
        let saturation = percent(parts[1])?;
        let lightness = percent(parts[2])?;
        let amplitude = saturation * lightness.min(1. - lightness);
        let channel = |n: f64| {
            let k = (n + h) % 12.;
            (lightness - amplitude * (-1f64).max((k - 3.).min(9. - k).min(1.))) * 255.
        };
        return Some([channel(0.), channel(8.), channel(4.), a]);
    }
    let hex = NAMED_COLORS.split_whitespace().find_map(|pair| {
        let (name, hex) = pair.split_once(':')?;
        (name == s).then_some(hex)
    })?;
    parse_color(&format!("#{hex}"))
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

    #[test]
    fn nested_assets_quotes_gradients_markers_and_inherited_colors() {
        let source = r#"<svg style='--a:var(--b);--b:#123;color:var(--a)'><defs><marker id='arrow'><path stroke='currentColor'/></marker><linearGradient id='gradient'><stop style='stop-color:currentColor;stop-opacity:.5'/></linearGradient></defs><svg viewBox = '0 0 24 24' style='--b:#456'><path fill='url(#gradient)' marker-end='url(#arrow)' data-name='a &amp; b &quot;c&quot;' stroke='currentColor'/><text style='font-family:"A; B"'>x</text></svg><!-- > ignored --><path fill='currentColor'/></svg>"#;
        let output = inline_styles(source, &[]);
        assert!(output.contains("viewBox=\"0 0 24 24\""));
        assert!(output.contains("stroke=\"#123\""));
        assert!(output.contains("stop-color=\"#123\" stop-opacity=\".5\""));
        assert!(output.contains("data-name=\"a &amp; b &quot;c&quot;\""));
        assert!(output.contains("marker-end=\"url(#arrow)\""));
        assert!(output.contains("font-family=\"&quot;A; B&quot;\""));
        assert!(output.contains("<!-- > ignored --><path fill=\"#123\"/>"));
        assert!(!output.contains("var("));
    }
    #[test]
    fn premultiplied_colors_and_percentages() {
        for (input, expected) in [
            ("in srgb, red 50%, transparent", "#ff000080"),
            ("in srgb, rgb(255 0 0 / 20%) 25%, blue 75%", "#1000efcc"),
            ("in srgb, red 20%, blue 20%", "#80008066"),
            ("in srgb, red 80%, blue 80%", "#800080"),
            ("in srgb, transparent, transparent", "#00000000"),
            ("in srgb, hsl(120 100% 50%) 50%, rebeccapurple", "#33994d"),
            ("in srgb, color-mix(in srgb, red, blue), white 25%", "#9f409f"),
        ] {
            assert_eq!(color_mix(input, "black").as_deref(), Some(expected), "{input}");
        }
        for input in ["in srgb, red 0%, blue 0%", "in srgb, red -1%, blue", "in srgb, red 101%, blue"] {
            assert_eq!(color_mix(input, "black"), None, "{input}");
        }
    }

    #[test]
    fn css_color_forms_and_invalid_unicode() {
        for (input, expected) in [
            ("RebeccaPurple", "#663399"),
            ("grey", "#808080"),
            ("hsl(-240deg 100% 50%)", "#00ff00"),
            ("hsla(200grad, 100%, 50%, 50%)", "#00ffff80"),
            ("hsl(0.5turn 100% 50% / .25)", "#00ffff40"),
            ("hsl(3.141592653589793rad 100% 50%)", "#00ffff"),
            ("rgb(300 -10 0 / 150%)", "#ff0000"),
            ("rgb(100% 0% 0% / 25%)", "#ff000040"),
        ] {
            assert_eq!(parse_color(input).map(format_color).as_deref(), Some(expected), "{input}");
        }
        for input in ["#é123", "#ééé", "#😀ab", "#aé", "#xyz", "rgb(NaN 0 0)", "hsl(inf 20% 30%)"] {
            assert_eq!(parse_color(input), None, "{input}");
        }
    }

    #[test]
    fn nested_expressions_current_color_and_gradient_styles() {
        let source = r#"<svg color='red' style='--tone:hsl(240 100% 50%);--third:calc(1px / 3)'><defs><linearGradient id='g'><stop style='stop-color:color-mix(in srgb,currentColor 20%, transparent)'/></linearGradient></defs><g style='fill:color-mix(in srgb,currentColor 50%,var(--tone));color:lime'><path stroke='color-mix(in srgb,color-mix(in srgb,red,blue),white 25%)' style='stroke-width:calc(var(--third) * 3)' data-name='currentColor'/></g></svg>"#;
        let output = inline_styles(source, &[]);
        assert!(output.contains("stop-color=\"#ff000033\""), "{output}");
        assert!(output.contains("fill=\"#008080\" color=\"lime\""), "{output}");
        assert!(output.contains("stroke=\"#9f409f\""), "{output}");
        assert!(output.contains("stroke-width=\"1\""), "{output}");
        assert!(output.contains("data-name=\"currentColor\""), "{output}");
        assert!(!output.contains("color-mix("));
        assert!(!output.contains("calc("));
        assert_eq!(calc("2 * (3px + 4px) / 2"), Some(7.));
        assert_eq!(calc("calc(1px / 3) * 3"), Some(1.));
        assert_eq!(calc("1px / 0"), None);
        assert_eq!(calc("1px + unknown"), None);
    }
}
