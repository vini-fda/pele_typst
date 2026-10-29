//! Git history graphs with deterministic commit identifiers.
mod render;
use crate::diagrams::tokens::*;
use crate::errors::{PeleError, PeleErrorCode, syntax_error};
use crate::preprocess::Config;
use crate::util::{is_js_space, is_word16, js_number, ord, trim_start};
use indexmap::IndexMap;
pub use render::render;

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub seq: usize,
    pub kind: usize,
    pub tags: Vec<String>,
    pub parents: Vec<String>,
    pub branch: String,
    pub custom_type: Option<usize>,
    pub custom_id: Option<bool>,
}
#[derive(Debug, Clone)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub direction: String,
    pub commits: IndexMap<String, Commit>,
    pub branches: IndexMap<String, Option<String>>,
    pub lanes: Vec<String>,
    pub current_branch: String,
    pub head: Option<Commit>,
    pub warnings: Vec<String>,
}
#[derive(Default)]
struct Statement {
    kind: usize,
    id: Option<String>,
    message: String,
    tags: Vec<String>,
    commit_type: Option<usize>,
    name: String,
    order: Option<f64>,
    parent: Option<String>,
}
const NAMES: &[&str] = &[
    "cherry-pick",
    "gitGraph:",
    "HIGHLIGHT",
    "gitGraph",
    "checkout",
    "REVERSE",
    "parent:",
    "commit",
    "NORMAL",
    "branch",
    "order:",
    "switch",
    "type:",
    "merge",
    "msg:",
    "tag:",
    "id:",
    "LR",
    "TB",
    "BT",
    ":",
    "ACC_DESCR",
    "ACC_TITLE",
    "TITLE",
    "INT",
    "STRING",
    "NEWLINE",
    "WHITESPACE",
    "YAML",
    "DIRECTIVE",
    "SINGLE_LINE_COMMENT",
    "REFERENCE",
];
fn at(s: &[u16], p: usize) -> u32 {
    s.get(p).map_or(u32::MAX, |&c| c as u32)
}
fn starts(s: &[u16], p: usize, t: &str) -> bool {
    t.encode_utf16().enumerate().all(|(i, c)| s.get(p + i) == Some(&c))
}
fn reference(s: &[u16], p: usize) -> Option<usize> {
    if !is_word16(at(s, p)) {
        return None;
    }
    let mut end = p + 1;
    let mut last = end;
    while is_word16(at(s, end)) || matches!(at(s, end), 45..=47) {
        if is_word16(at(s, end)) || at(s, end) == 45 {
            last = end + 1;
        }
        end += 1;
    }
    Some(last)
}
fn integer(s: &[u16], p: usize) -> Option<usize> {
    if at(s, p) == 48 {
        return Some(p + 1);
    }
    if !(49..=57).contains(&at(s, p)) {
        return None;
    }
    let mut end = p + 1;
    while (48..=57).contains(&at(s, end)) {
        end += 1;
    }
    if at(s, end) == 46 {
        end -= 1;
    }
    (end > p).then_some(end)
}
fn lex(source: &str) -> Result<Tokens, PeleError> {
    let mut out = Tokens {
        kinds: vec![],
        starts: vec![],
        ends: vec![],
        src: source.encode_utf16().collect(),
    };
    let mut p = 0;
    while p < out.src.len() {
        let s = &out.src;
        let mut hit = None;
        for (k, &word) in NAMES.iter().take(21).enumerate() {
            if starts(s, p, word) {
                let end = p + word.len();
                let is_word = matches!(k, 0 | 2 | 3 | 4 | 5 | 7 | 8 | 9 | 11 | 13 | 17 | 18 | 19);
                let longer = is_word && reference(s, p).is_some_and(|e| e > end);
                let boundary = k != 3 || end == s.len() || crate::util::is_space16(at(s, end)) || starts(s, end, "%%");
                if !longer && boundary {
                    hit = Some((k as i32, end));
                    break;
                }
            }
        }
        if hit.is_none() {
            hit = acc_descr(s, p).map(|e| (21, e));
        }
        if hit.is_none() {
            hit = acc_title(s, p).map(|e| (22, e));
        }
        if hit.is_none() {
            let word_start = (p..s.len()).find(|&i| !matches!(s[i], 9 | 32)).unwrap_or(s.len());
            hit = title(s, p)
                .filter(|_| reference(s, word_start).is_none_or(|e| e <= word_start + 5))
                .map(|e| (23, e));
        }
        if hit.is_none() {
            hit = integer(s, p).map(|e| (24, e));
        }
        if hit.is_none() {
            hit = string(s, p).map(|e| (25, e));
        }
        if hit.is_none() {
            hit = newline(s, p).map(|e| (26, e));
        }
        if let Some((k, end)) = hit {
            out.kinds.push(k);
            out.starts.push(p);
            out.ends.push(end);
            p = end;
            continue;
        }
        if let Some(end) = whitespace(s, p)
            .or_else(|| yaml(s, p))
            .or_else(|| directive(s, p))
            .or_else(|| comment(s, p))
        {
            p = end;
            continue;
        }
        if let Some(end) = reference(s, p) {
            out.kinds.push(31);
            out.starts.push(p);
            out.ends.push(end);
            p = end;
            continue;
        }
        return Err(syntax_error("gitGraph", s, p, "", true));
    }
    out.kinds.push(-1);
    out.starts.push(p);
    out.ends.push(p);
    Ok(out)
}
fn eol(r: &mut Reader) -> Result<(), PeleError> {
    if r.kind() != -1 && r.kind() != 26 {
        return Err(r.fail("a line break or the end of input"));
    }
    while r.accept(26) {}
    Ok(())
}
fn name(r: &mut Reader) -> Result<String, PeleError> {
    match r.kind() {
        31 => Ok(r.take()),
        25 => Ok(string_value(&r.take())),
        _ => Err(r.fail("a branch name")),
    }
}
fn text(r: &mut Reader) -> Result<String, PeleError> {
    Ok(string_value(&r.expect(25)?))
}
fn commit_type(r: &mut Reader) -> Result<usize, PeleError> {
    let kind = r.kind();
    if !matches!(kind, 8 | 5 | 2) {
        return Err(r.fail("NORMAL, REVERSE, or HIGHLIGHT"));
    }
    r.i += 1;
    Ok(match kind {
        5 => 1,
        2 => 2,
        _ => 0,
    })
}
fn invalid(s: impl Into<String>) -> PeleError {
    PeleError::new(s, PeleErrorCode::Semantic).of("gitGraph")
}
fn next_id(seq: usize, commits: &IndexMap<String, Commit>) -> String {
    let mut salt = (seq + 1) as u32;
    loop {
        let hex = format!("{:08x}", salt.wrapping_mul(0x9e3779b1));
        let id = format!("{seq}-{}", &hex[..7]);
        if !commits.contains_key(&id) {
            return id;
        }
        salt = salt.wrapping_add(0x9e3779b1);
    }
}
pub fn parse(source: &str, config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let types: Vec<TokenType> = NAMES
        .iter()
        .map(|&name| TokenType {
            name,
            pattern: Pattern::Literal(name),
            hidden: false,
            first: None,
        })
        .collect();
    let mut r = Reader {
        i: 0,
        tokens: lex(source)?,
        types: &types,
        diagram: "gitGraph",
    };
    let mut given_title = None;
    let mut acc_title = None;
    let mut acc_descr = None;
    let mut direction = "LR".to_string();
    let mut statements = vec![];
    while r.accept(26) {}
    if !r.accept(1) {
        r.expect(3)?;
        if matches!(r.kind(), 17..=19) && r.tokens.kinds.get(r.i + 1) == Some(&20) {
            direction = r.take();
            r.i += 1;
        } else {
            r.accept(20);
        }
    }
    while r.kind() != -1 {
        let k = r.kind();
        match k {
            26 => r.i += 1,
            21 => {
                acc_descr = Some(acc_descr_value(&r.take()));
                eol(&mut r)?;
            }
            22 => {
                acc_title = Some(acc_title_value(&r.take()));
                eol(&mut r)?;
            }
            23 => {
                given_title = Some(title_value(&r.take()));
                eol(&mut r)?;
            }
            7 | 9 | 13 | 4 | 11 | 0 => {
                r.i += 1;
                let mut node = Statement {
                    kind: k as usize,
                    ..Default::default()
                };
                match k {
                    7 => loop {
                        if r.accept(16) {
                            node.id = Some(text(&mut r)?);
                        } else if r.accept(15) {
                            node.tags.push(text(&mut r)?);
                        } else if r.accept(12) {
                            node.commit_type = Some(commit_type(&mut r)?);
                        } else if r.accept(14) || r.kind() == 25 {
                            node.message = text(&mut r)?;
                        } else {
                            break;
                        }
                    },
                    9 => {
                        node.name = name(&mut r)?;
                        if r.accept(10) {
                            node.order = Some(js_number(&r.expect(24)?));
                        }
                    }
                    13 => {
                        node.name = name(&mut r)?;
                        loop {
                            if r.accept(16) {
                                node.id = Some(text(&mut r)?);
                            } else if r.accept(15) {
                                node.tags.push(text(&mut r)?);
                            } else if r.accept(12) {
                                node.commit_type = Some(commit_type(&mut r)?);
                            } else {
                                break;
                            }
                        }
                    }
                    4 | 11 => node.name = name(&mut r)?,
                    0 => loop {
                        if r.accept(16) {
                            node.id = Some(text(&mut r)?);
                        } else if r.accept(15) {
                            node.tags.push(text(&mut r)?);
                        } else if r.accept(6) {
                            node.parent = Some(text(&mut r)?);
                        } else {
                            break;
                        }
                    },
                    _ => unreachable!(),
                }
                eol(&mut r)?;
                statements.push(node);
            }
            _ => return Err(r.fail("a statement, a title, or a line break")),
        }
    }
    let git = config.get("gitGraph");
    let main = git
        .and_then(|g| g.get("mainBranchName"))
        .and_then(|v| v.as_str())
        .unwrap_or("main")
        .to_string();
    let main_order = git.and_then(|g| g.get("mainBranchOrder")).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mut model = Model {
        title: given_title.filter(|v| !v.is_empty()).or_else(|| title.map(String::from)),
        acc_title: acc_title.filter(|v| !v.is_empty()).map(|v| trim_start(&v).into()),
        acc_descr: acc_descr.filter(|v| !v.is_empty()).map(|v| {
            let mut out = String::new();
            let mut chars = v.chars().peekable();
            while let Some(c) = chars.next() {
                out.push(c);
                if c == '\n' {
                    while chars.peek().is_some_and(|&c| is_js_space(c)) {
                        chars.next();
                    }
                }
            }
            out
        }),
        direction,
        commits: IndexMap::new(),
        branches: IndexMap::from([(main.clone(), None)]),
        lanes: vec![],
        current_branch: main.clone(),
        head: None,
        warnings: vec![],
    };
    let mut orders = IndexMap::from([(main, main_order)]);
    let mut seq = 0;
    for s in statements {
        let current = model.current_branch.clone();
        let parent = model.head.as_ref().map(|c| c.id.clone());
        let new = match s.kind {
            7 => {
                let id = s.id.filter(|v| !v.is_empty()).unwrap_or_else(|| next_id(seq, &model.commits));
                if model.commits.contains_key(&id) {
                    model.warnings.push(format!("Commit ID {id} already exists"));
                }
                Some(Commit {
                    id,
                    message: s.message,
                    seq,
                    kind: s.commit_type.unwrap_or(0),
                    tags: s.tags,
                    parents: parent.into_iter().collect(),
                    branch: current.clone(),
                    custom_type: None,
                    custom_id: None,
                })
            }
            9 => {
                if model.branches.contains_key(&s.name) {
                    return Err(invalid(format!(
                        "Trying to create an existing branch. (Help: Either use a new name if you want create a new branch or try using \"checkout {}\")",
                        s.name
                    )));
                }
                model.branches.insert(s.name.clone(), parent);
                orders.insert(s.name.clone(), s.order.unwrap_or(0.0));
                model.current_branch = s.name.clone();
                model.head = model
                    .branches
                    .get(&s.name)
                    .and_then(|v| v.as_ref())
                    .filter(|s| !s.is_empty())
                    .and_then(|id| model.commits.get(id))
                    .cloned();
                None
            }
            4 | 11 => {
                if !model.branches.contains_key(&s.name) {
                    return Err(invalid(format!(
                        "Trying to checkout branch which is not yet created. (Help try using \"branch {}\")",
                        s.name
                    )));
                }
                model.current_branch = s.name.clone();
                model.head = model
                    .branches
                    .get(&s.name)
                    .and_then(|v| v.as_ref())
                    .filter(|s| !s.is_empty())
                    .and_then(|id| model.commits.get(id))
                    .cloned();
                None
            }
            13 => {
                let other = s.name;
                let current_head = model.branches.get(&current).and_then(|s| s.as_ref()).filter(|s| !s.is_empty());
                let other_head = model.branches.get(&other).and_then(|s| s.as_ref()).filter(|s| !s.is_empty());
                let current_commit = current_head.and_then(|id| model.commits.get(id));
                let other_commit = other_head.and_then(|id| model.commits.get(id));
                if current_commit.is_some_and(|c| c.branch == other) && other_commit.is_some() {
                    return Err(invalid(format!("Cannot merge branch '{other}' into itself.")));
                }
                if current == other {
                    return Err(invalid("Incorrect usage of \"merge\". Cannot merge a branch to itself"));
                }
                if current_commit.is_none() {
                    return Err(invalid(format!(
                        "Incorrect usage of \"merge\". Current branch ({current})has no commits"
                    )));
                }
                if !model.branches.contains_key(&other) {
                    return Err(invalid(format!(
                        "Incorrect usage of \"merge\". Branch to be merged ({other}) does not exist"
                    )));
                }
                if other_commit.is_none() {
                    return Err(invalid(format!(
                        "Incorrect usage of \"merge\". Branch to be merged ({other}) has no commits"
                    )));
                }
                if current_head == other_head {
                    return Err(invalid("Incorrect usage of \"merge\". Both branches have same head"));
                }
                let custom = s.id.filter(|v| !v.is_empty());
                if custom.as_ref().is_some_and(|id| model.commits.contains_key(id)) {
                    return Err(invalid(format!(
                        "Incorrect usage of \"merge\". Commit with id:{} already exists, use different custom id",
                        custom.unwrap()
                    )));
                }
                let custom_id = custom.is_some();
                let parents = if let Some(parent) = parent {
                    vec![parent, other_head.cloned().unwrap_or_default()]
                } else {
                    vec![]
                };
                Some(Commit {
                    id: custom.unwrap_or_else(|| next_id(seq, &model.commits)),
                    message: format!("merged branch {other} into {current}"),
                    seq,
                    parents,
                    branch: current.clone(),
                    kind: 3,
                    custom_type: s.commit_type,
                    custom_id: Some(custom_id),
                    tags: s.tags,
                })
            }
            0 => {
                let Some(source) = s.id.as_ref().filter(|v| !v.is_empty()).and_then(|id| model.commits.get(id)) else {
                    return Err(invalid(
                        "Incorrect usage of \"cherryPick\". Source commit id should exist and provided",
                    ));
                };
                let picked_parent = s.parent.as_ref().filter(|v| !v.is_empty());
                if picked_parent.is_some_and(|p| !source.parents.contains(p)) {
                    return Err(invalid(
                        "Invalid operation: The specified parent commit is not an immediate parent of the cherry-picked commit.",
                    ));
                }
                if source.kind == 3 && picked_parent.is_none() {
                    return Err(invalid(
                        "Incorrect usage of cherry-pick: If the source commit is a merge commit, an immediate parent commit must be specified.",
                    ));
                }
                if source.branch == current {
                    return Err(invalid(
                        "Incorrect usage of \"cherryPick\". Source commit is already on current branch",
                    ));
                }
                if model
                    .branches
                    .get(&current)
                    .and_then(|v| v.as_ref())
                    .filter(|v| !v.is_empty())
                    .is_none_or(|id| !model.commits.contains_key(id))
                {
                    return Err(invalid(format!(
                        "Incorrect usage of \"cherry-pick\". Current branch ({current})has no commits"
                    )));
                }
                let tags = if s.tags.is_empty() {
                    vec![format!(
                        "cherry-pick:{}{}",
                        source.id,
                        if source.kind == 3 {
                            format!("|parent:{}", picked_parent.unwrap())
                        } else {
                            String::new()
                        }
                    )]
                } else {
                    s.tags.into_iter().filter(|v| !v.is_empty()).collect()
                };
                Some(Commit {
                    id: next_id(seq, &model.commits),
                    message: format!("cherry-picked {} into {current}", source.message),
                    seq,
                    parents: if let Some(p) = parent { vec![p, source.id.clone()] } else { vec![] },
                    branch: current.clone(),
                    kind: 4,
                    tags,
                    custom_type: None,
                    custom_id: None,
                })
            }
            _ => unreachable!(),
        };
        if let Some(commit) = new {
            seq += 1;
            model.head = Some(commit.clone());
            model.branches.insert(current, Some(commit.id.clone()));
            model.commits.insert(commit.id.clone(), commit);
        }
    }
    let mut lanes: Vec<_> = orders.into_iter().collect();
    lanes.sort_by(|a, b| ord(a.1 - b.1));
    model.lanes = lanes.into_iter().map(|v| v.0).collect();
    Ok(model)
}
