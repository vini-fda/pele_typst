mod render;
use crate::PeleError;
use crate::diagrams::timeline::story;
use crate::diagrams::tokens::{self, Pattern, Reader, TokenType};
use crate::errors::PeleErrorCode;
use crate::preprocess::Config;
use crate::util::{is_space16, is_word16, js_num, trim};
use indexmap::IndexMap;
pub use render::render;
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub class_name: String,
    pub ox: Option<f64>,
    pub oy: Option<f64>,
    pub in_pipeline: bool,
    pub pipeline_parent: bool,
    pub inertia: bool,
    pub strategy: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Link {
    pub source: String,
    pub target: String,
    pub dashed: bool,
    pub label: Option<String>,
    pub flow: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Trend {
    pub node_id: String,
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug)]
pub struct Pipeline {
    pub node_id: String,
    pub components: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Annotation {
    pub number: f64,
    pub x: f64,
    pub y: f64,
    pub text: String,
}
#[derive(Clone, Debug)]
pub struct Note {
    pub text: String,
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug)]
pub struct Force {
    pub name: String,
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub size: Option<(f64, f64)>,
    pub stages: Vec<String>,
    pub boundaries: Vec<f64>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub trends: Vec<Trend>,
    pub pipelines: Vec<Pipeline>,
    pub annotations: Vec<Annotation>,
    pub notes: Vec<Note>,
    pub accelerators: Vec<Force>,
    pub deaccelerators: Vec<Force>,
    pub annotations_box: Option<(f64, f64)>,
}
fn starts(s: &[u16], p: usize, t: &str) -> bool {
    t.encode_utf16().enumerate().all(|(i, c)| s.get(p + i) == Some(&c))
}
fn digit(s: &[u16], p: usize) -> bool {
    s.get(p).is_some_and(|c| matches!(c, 48..=57))
}
fn number(s: &[u16], p: usize) -> Option<usize> {
    let mut q = p;
    while digit(s, q) {
        q += 1
    }
    if q == p || s.get(q) != Some(&46) {
        return None;
    }
    q += 1;
    let start = q;
    while digit(s, q) {
        q += 1
    }
    (q > start).then_some(q)
}
fn int(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) == Some(&48) {
        return Some(p + 1);
    }
    if !digit(s, p) {
        return None;
    }
    let mut q = p + 1;
    while digit(s, q) {
        q += 1
    }
    if s.get(q) == Some(&46) {
        q -= 1
    }
    (q > p).then_some(q)
}
fn arrow(s: &[u16], p: usize) -> Option<usize> {
    for t in ["-->", "-.->", ">"] {
        if starts(s, p, t) {
            return Some(p + t.len());
        }
    }
    if !starts(s, p, "+'") {
        return None;
    }
    let q = (p + 2..s.len()).find(|&i| s[i] == 39)? + 1;
    for t in ["<>", "<", ">"] {
        if starts(s, q, t) {
            return Some(q + t.len());
        }
    }
    None
}
fn minus(s: &[u16], p: usize) -> Option<usize> {
    (s.get(p) == Some(&45) && !starts(s, p, "->") && arrow(s, p).is_none() && tokens::yaml(s, p).is_none()).then_some(p + 1)
}
fn port(s: &[u16], p: usize) -> Option<usize> {
    ["+<>", "+>", "+<"].iter().find(|t| starts(s, p, t)).map(|t| p + t.len())
}
fn link_label(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&59) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|c| !matches!(c, 10 | 13)) {
        q += 1
    }
    (q > p + 1).then_some(q)
}
fn strategy(s: &[u16], p: usize) -> Option<usize> {
    ["build", "buy", "outsource", "market"]
        .iter()
        .find(|t| starts(s, p, t))
        .map(|t| p + t.len())
}
fn alpha(c: u16) -> bool {
    matches!(c,65..=90|97..=122)
}
fn name(s: &[u16], p: usize) -> Option<usize> {
    if starts(s, p, "accTitle")
        || starts(s, p, "accDescr")
        || (starts(s, p, "title") && s.get(p + 5).is_some_and(|c| is_space16(*c as u32)))
        || !s.get(p).is_some_and(|c| alpha(*c))
    {
        return None;
    }
    let part = |i: usize| {
        s.get(i)
            .is_some_and(|c| is_word16(*c as u32) || matches!(c, 40 | 41 | 38) || (*c == 45 && s.get(i + 1) != Some(&62)))
    };
    let mut q = p + 1;
    while part(q) {
        q += 1
    }
    loop {
        let mut n = q;
        while s.get(n).is_some_and(|c| matches!(c, 9 | 32)) {
            n += 1
        }
        if n == q || !s.get(n).is_some_and(|c| alpha(*c) || *c == 40) {
            break;
        }
        q = n + 1;
        while part(q) {
            q += 1
        }
    }
    Some(q)
}
fn ws(s: &[u16], p: usize) -> Option<usize> {
    let mut q = p;
    while s.get(q).is_some_and(|c| matches!(c, 9 | 32)) {
        q += 1
    }
    (q > p).then_some(q)
}
fn id(s: &[u16], p: usize) -> Option<usize> {
    if !s.get(p).is_some_and(|c| is_word16(*c as u32)) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|c| is_word16(*c as u32) || *c == 45) {
        q += 1
    }
    while q > p + 1 && s[q - 1] == 45 {
        q -= 1
    }
    Some(q)
}
fn token(name: &'static str, m: tokens::Matcher, hidden: bool) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(m),
        hidden,
        first: None,
    }
}
fn literal(name: &'static str, s: &'static str) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Literal(s),
        hidden: false,
        first: None,
    }
}
fn types() -> Vec<TokenType> {
    let mut v = vec![
        literal("[", "["),
        literal(",", ","),
        literal("]", "]"),
        literal("@", "@"),
        literal("/", "/"),
        literal("(", "("),
        literal(")", ")"),
        token("-", minus, false),
        literal("{", "{"),
        literal("}", "}"),
        token("WARDLEY_NUMBER", number, false),
        literal("ARROW", "->"),
        token("LINK_PORT", port, false),
        token("LINK_ARROW", arrow, false),
        token("LINK_LABEL", link_label, false),
        token("STRATEGY", strategy, false),
    ];
    for (n, t) in [
        ("KW_WARDLEY", "wardley-beta"),
        ("KW_SIZE", "size"),
        ("KW_EVOLUTION", "evolution"),
        ("KW_ANCHOR", "anchor"),
        ("KW_COMPONENT", "component"),
        ("KW_LABEL", "label"),
        ("KW_INERTIA", "inertia"),
        ("KW_EVOLVE", "evolve"),
        ("KW_PIPELINE", "pipeline"),
        ("KW_NOTE", "note"),
        ("KW_ANNOTATIONS", "annotations"),
        ("KW_ANNOTATION", "annotation"),
        ("KW_ACCELERATOR", "accelerator"),
        ("KW_DEACCELERATOR", "deaccelerator"),
    ] {
        v.push(literal(n, t))
    }
    v.extend([
        token("NAME_WITH_SPACES", name, false),
        token("WS", ws, true),
        token("ACC_DESCR", tokens::acc_descr, false),
        token("ACC_TITLE", tokens::acc_title, false),
        token("TITLE", tokens::title, false),
        token("INT", int, false),
        token("STRING", tokens::string, false),
        token("ID", id, false),
        token("NEWLINE", tokens::newline, false),
        token("WHITESPACE", tokens::whitespace, true),
        token("YAML", tokens::yaml, true),
        token("DIRECTIVE", tokens::directive, true),
        token("SINGLE_LINE_COMMENT", tokens::comment, true),
    ]);
    v
}
fn kind(r: &Reader) -> &'static str {
    if r.kind() == -1 { "" } else { r.types[r.kind() as usize].name }
}
fn expect(r: &mut Reader, name: &str) -> Result<String, PeleError> {
    let id = r.types.iter().position(|t| t.name == name).unwrap();
    r.expect(id as i32)
}
fn accept(r: &mut Reader, name: &str) -> bool {
    if kind(r) == name {
        r.i += 1;
        true
    } else {
        false
    }
}
fn read_name(r: &mut Reader) -> Result<String, PeleError> {
    match kind(r) {
        "STRING" => Ok(tokens::string_value(&r.take())),
        "ID" | "NAME_WITH_SPACES" => Ok(r.take()),
        _ => Err(r.fail("a name")),
    }
}
fn value(r: &mut Reader, mode: u8) -> Result<f64, PeleError> {
    let token = if mode == 0 || mode == 2 && kind(r) == "WARDLEY_NUMBER" {
        "WARDLEY_NUMBER"
    } else {
        "INT"
    };
    Ok(expect(r, token)?.parse().unwrap_or(f64::INFINITY))
}
fn pair(r: &mut Reader, mode: u8) -> Result<(f64, f64), PeleError> {
    expect(r, "[")?;
    let a = value(r, mode)?;
    expect(r, ",")?;
    let b = value(r, mode)?;
    expect(r, "]")?;
    Ok((a, b))
}
fn label(r: &mut Reader) -> Result<(Option<f64>, Option<f64>), PeleError> {
    if !accept(r, "KW_LABEL") {
        return Ok((None, None));
    }
    expect(r, "[")?;
    let neg = accept(r, "-");
    let x = value(r, 1)? * if neg { -1.0 } else { 1.0 };
    expect(r, ",")?;
    let neg = accept(r, "-");
    let y = value(r, 1)? * if neg { -1.0 } else { 1.0 };
    expect(r, "]")?;
    Ok((Some(x), Some(y)))
}
fn eol(r: &mut Reader) -> Result<(), PeleError> {
    if r.kind() == -1 {
        return Ok(());
    }
    if kind(r) != "NEWLINE" {
        return Err(r.fail("a line break or the end of input"));
    }
    while accept(r, "NEWLINE") {}
    Ok(())
}
fn semantic(s: String) -> PeleError {
    PeleError::new(s, PeleErrorCode::Semantic).of("wardley")
}
fn percent(v: f64, c: &str) -> Result<f64, PeleError> {
    let n = if v <= 1.0 { v * 100.0 } else { v };
    if n < 0.0 || n > 100.0 {
        Err(semantic(format!(
            "{c} must be between 0-1 (decimal) or 0-100 (percentage). Received: {}",
            js_num(v)
        )))
    } else {
        Ok(n)
    }
}
fn coords(a: f64, b: f64, c: &str) -> Result<(f64, f64), PeleError> {
    Ok((percent(b, &format!("{c} evolution"))?, percent(a, &format!("{c} visibility"))?))
}
fn flow(port: Option<&str>) -> Option<String> {
    match port {
        Some("+<>") => Some("bidirectional".into()),
        Some("+<") => Some("backward".into()),
        Some("+>") => Some("forward".into()),
        _ => None,
    }
}
fn add_node(nodes: &mut IndexMap<String, Node>, mut node: Node) {
    if let Some(old) = nodes.get(&node.id) {
        node.ox = node.ox.or(old.ox);
        node.oy = node.oy.or(old.oy);
        node.in_pipeline |= old.in_pipeline;
        node.pipeline_parent |= old.pipeline_parent;
    }
    nodes.insert(node.id.clone(), node);
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let ts = types();
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut r = Reader {
        i: 0,
        tokens: tokens::tokenize(&src, &ts, "wardley")?,
        types: &ts,
        diagram: "wardley",
    };
    let mut out = Model {
        title: title.map(str::to_string),
        ..Default::default()
    };
    let mut anchors = Vec::new();
    let mut components = Vec::new();
    let mut pipes = Vec::new();
    let mut links = Vec::new();
    let mut evolves = Vec::new();
    let mut notes = Vec::new();
    let mut boxes = Vec::new();
    let mut annotations = Vec::new();
    let mut forces = Vec::new();
    while accept(&mut r, "NEWLINE") {}
    expect(&mut r, "KW_WARDLEY")?;
    while r.kind() != -1 {
        match kind(&r) {
            "NEWLINE" => r.i += 1,
            "ACC_DESCR" => {
                out.acc_descr = Some(story::squeeze(&tokens::acc_descr_value(&r.take())));
                eol(&mut r)?
            }
            "ACC_TITLE" => {
                out.acc_title = Some(story::acc(&tokens::acc_title_value(&r.take())));
                eol(&mut r)?
            }
            "TITLE" => {
                let t = tokens::title_value(&r.take());
                out.title = if t.is_empty() { title.map(str::to_string) } else { Some(t) };
                eol(&mut r)?
            }
            "KW_SIZE" => {
                r.i += 1;
                out.size = Some(pair(&mut r, 1)?);
                eol(&mut r)?
            }
            "KW_EVOLUTION" => {
                r.i += 1;
                out.stages.clear();
                out.boundaries.clear();
                loop {
                    let name = read_name(&mut r)?;
                    if accept(&mut r, "@") {
                        out.boundaries.push(value(&mut r, 0)?)
                    }
                    let stage = if accept(&mut r, "/") {
                        format!("{} / {}", trim(&name), trim(&read_name(&mut r)?))
                    } else {
                        trim(&name).into()
                    };
                    out.stages.push(stage);
                    if out.stages.len() == 1 {
                        expect(&mut r, "ARROW")?;
                    } else if !accept(&mut r, "ARROW") {
                        break;
                    }
                }
                eol(&mut r)?
            }
            "KW_ANCHOR" => {
                r.i += 1;
                let n = read_name(&mut r)?;
                let (a, b) = pair(&mut r, 0)?;
                anchors.push((n, a, b));
                eol(&mut r)?
            }
            "KW_COMPONENT" => {
                r.i += 1;
                let n = read_name(&mut r)?;
                let (a, b) = pair(&mut r, 0)?;
                let (ox, oy) = label(&mut r)?;
                let strategy = if kind(&r) == "("
                    && r.tokens
                        .kinds
                        .get(r.i + 1)
                        .is_some_and(|&k| k >= 0 && ts[k as usize].name == "STRATEGY")
                {
                    r.i += 1;
                    let s = r.take();
                    expect(&mut r, ")")?;
                    Some(s)
                } else {
                    None
                };
                let inertia = if accept(&mut r, "KW_INERTIA") {
                    true
                } else if accept(&mut r, "(") {
                    expect(&mut r, "KW_INERTIA")?;
                    expect(&mut r, ")")?;
                    true
                } else {
                    false
                };
                components.push((n, a, b, ox, oy, strategy, inertia));
                eol(&mut r)?
            }
            "KW_EVOLVE" => {
                r.i += 1;
                let n = read_name(&mut r)?;
                let x = value(&mut r, 0)?;
                evolves.push((n, x));
                eol(&mut r)?
            }
            "KW_PIPELINE" => {
                r.i += 1;
                let n = read_name(&mut r)?;
                expect(&mut r, "{")?;
                expect(&mut r, "NEWLINE")?;
                while accept(&mut r, "NEWLINE") {}
                let mut children = Vec::new();
                loop {
                    expect(&mut r, "KW_COMPONENT")?;
                    let child = read_name(&mut r)?;
                    expect(&mut r, "[")?;
                    let x = value(&mut r, 0)?;
                    expect(&mut r, "]")?;
                    let (ox, oy) = label(&mut r)?;
                    eol(&mut r)?;
                    children.push((child, x, ox, oy));
                    if kind(&r) != "KW_COMPONENT" {
                        break;
                    }
                }
                expect(&mut r, "}")?;
                eol(&mut r)?;
                pipes.push((n, children));
            }
            "KW_NOTE" => {
                r.i += 1;
                let text = tokens::string_value(&expect(&mut r, "STRING")?);
                let (a, b) = pair(&mut r, 0)?;
                notes.push((text, a, b));
                eol(&mut r)?
            }
            "KW_ANNOTATIONS" => {
                r.i += 1;
                boxes.push(pair(&mut r, 2)?);
                eol(&mut r)?
            }
            "KW_ANNOTATION" => {
                r.i += 1;
                let n = value(&mut r, 1)?;
                expect(&mut r, ",")?;
                let (a, b) = pair(&mut r, 2)?;
                let text = tokens::string_value(&expect(&mut r, "STRING")?);
                annotations.push((n, a, b, text));
                eol(&mut r)?
            }
            "KW_ACCELERATOR" | "KW_DEACCELERATOR" => {
                let slow = kind(&r) == "KW_DEACCELERATOR";
                r.i += 1;
                let n = read_name(&mut r)?;
                let (a, b) = pair(&mut r, 0)?;
                forces.push((slow, n, a, b));
                eol(&mut r)?
            }
            _ => {
                if !matches!(kind(&r), "STRING" | "ID" | "NAME_WITH_SPACES") {
                    return Err(r.fail("a statement"));
                }
                let from = read_name(&mut r)?;
                let from_port = if kind(&r) == "LINK_PORT" { Some(r.take()) } else { None };
                let arrow = if matches!(kind(&r), "LINK_ARROW" | "ARROW") {
                    Some(r.take())
                } else {
                    None
                };
                let to = read_name(&mut r)?;
                let to_port = if kind(&r) == "LINK_PORT" { Some(r.take()) } else { None };
                let label = if kind(&r) == "LINK_LABEL" {
                    Some(trim(&r.take()[1..]).into())
                } else {
                    None
                };
                eol(&mut r)?;
                links.push((from, to, from_port, to_port, arrow, label));
            }
        }
    }
    let mut nodes = IndexMap::new();
    for (n, a, b) in anchors {
        let (x, y) = coords(a, b, &format!("Anchor \"{n}\""))?;
        add_node(
            &mut nodes,
            Node {
                id: n.clone(),
                label: n,
                x,
                y,
                class_name: "anchor".into(),
                ..Default::default()
            },
        )
    }
    for (n, a, b, ox, oy, strategy, inertia) in components {
        let (x, y) = coords(a, b, &format!("Component \"{n}\""))?;
        add_node(
            &mut nodes,
            Node {
                id: n.clone(),
                label: n,
                x,
                y,
                class_name: "component".into(),
                ox,
                oy,
                strategy,
                inertia,
                ..Default::default()
            },
        )
    }
    for (text, a, b) in notes {
        let (x, y) = coords(a, b, &format!("Note \"{text}\""))?;
        out.notes.push(Note { text, x, y })
    }
    let mut pipelines = IndexMap::new();
    for (parent, children) in pipes {
        let Some(node) = nodes.get_mut(&parent) else {
            return Err(semantic(format!(
                "Pipeline \"{parent}\" must reference an existing component with coordinates."
            )));
        };
        let y = node.y;
        node.pipeline_parent = true;
        let mut members = Vec::new();
        for (name, v, ox, oy) in children {
            let x = percent(v, &format!("Pipeline component \"{name}\" evolution"))?;
            let id = format!("{parent}_{name}");
            add_node(
                &mut nodes,
                Node {
                    id: id.clone(),
                    label: name,
                    x,
                    y,
                    class_name: "pipeline-component".into(),
                    ox,
                    oy,
                    in_pipeline: true,
                    ..Default::default()
                },
            );
            members.push(id)
        }
        pipelines.insert(
            parent.clone(),
            Pipeline {
                node_id: parent,
                components: members,
            },
        );
    }
    out.pipelines = pipelines.into_values().collect();
    let resolve = |s: String| {
        if nodes.contains_key(&s) {
            s
        } else {
            nodes.values().find(|n| n.label == s).map(|n| n.id.clone()).unwrap_or(s)
        }
    };
    for (from, to, fp, tp, arrow, label) in links {
        let a = arrow.as_deref().unwrap_or("");
        let quoted = if let Some(rest) = a.strip_prefix("+'") {
            rest.find('\'').map(|i| rest[..i].to_string())
        } else {
            None
        };
        let arrow_flow = if !a.starts_with('+') {
            None
        } else if a.contains("<>") {
            Some("bidirectional".into())
        } else if a.contains('<') {
            Some("backward".into())
        } else if a.contains('>') {
            Some("forward".into())
        } else {
            None
        };
        out.links.push(Link {
            source: resolve(from),
            target: resolve(to),
            dashed: a.contains("-.->") || a.contains(".-."),
            label: quoted.or(label),
            flow: flow(fp.as_deref()).or_else(|| flow(tp.as_deref())).or(arrow_flow),
        })
    }
    let mut trends = IndexMap::new();
    for (n, v) in evolves {
        if let Some(node) = nodes.get(&n) {
            let x = percent(v, &format!("Evolve target for \"{n}\""))?;
            trends.insert(n.clone(), Trend { node_id: n, x, y: node.y });
        }
    }
    out.trends = trends.into_values().collect();
    if let Some(&(a, b)) = boxes.first() {
        out.annotations_box = Some(coords(a, b, "Annotations box")?)
    }
    for (number, a, b, text) in annotations {
        let (x, y) = coords(a, b, &format!("Annotation {}", js_num(number)))?;
        out.annotations.push(Annotation { number, x, y, text })
    }
    for slow in [false, true] {
        for (_, name, a, b) in forces.iter().filter(|(s, _, _, _)| *s == slow) {
            let (x, y) = coords(
                *a,
                *b,
                &format!("{} \"{name}\"", if slow { "Deaccelerator" } else { "Accelerator" }),
            )?;
            let f = Force { name: name.clone(), x, y };
            if slow {
                out.deaccelerators.push(f)
            } else {
                out.accelerators.push(f)
            }
        }
    }
    out.nodes = nodes.into_values().collect();
    Ok(out)
}
