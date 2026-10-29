use super::lexer::{location, location_text};
use super::*;
use crate::errors::PeleErrorCode;
#[derive(Clone)]
struct Origin {
    kind: String,
    span: Span,
    generated: bool,
    label: Option<String>,
}
#[derive(Clone)]
struct State {
    item: Item,
    kind: String,
    parent: Option<Item>,
    actor_type: Option<String>,
    icon: Option<String>,
    business: Option<bool>,
    stereotype_span: Option<Span>,
}
#[derive(Clone)]
struct Group {
    item: Item,
    kind: Option<String>,
    members: Vec<String>,
}
fn unique_push(target: &mut Vec<String>, values: &[String]) {
    for v in values {
        if !target.contains(v) {
            target.push(v.clone())
        }
    }
}
fn err(src: &[u16], message: String, span: Span) -> PeleError {
    let (l, c, _) = location(src, span.0);
    let mut e = PeleError::new(message, PeleErrorCode::Semantic).of("usecase");
    e.line = l;
    e.column = c;
    e
}
fn fail(src: &[u16], msg: String, span: Span) -> PeleError {
    err(src, format!("{msg} at {}", location_text(src, span.0, span.1)), span)
}
fn conflict(src: &[u16], msg: String, cur: Span, prev: Span, cl: Option<&str>, pl: Option<&str>) -> PeleError {
    let suffix = |l: Option<&str>| l.map(|s| format!(" (label \"{s}\")")).unwrap_or_default();
    err(
        src,
        format!(
            "{msg} at {}{}; previous declaration at {}{}",
            location_text(src, cur.0, cur.1),
            suffix(cl),
            location_text(src, prev.0, prev.1),
            suffix(pl)
        ),
        cur,
    )
}
fn invalid(src: &[u16], id: &str, kind: &str, p: &Property) -> PeleError {
    fail(
        src,
        format!("Metadata property '{}' is invalid for {kind} '{id}'", p.key),
        p.key_span,
    )
}
fn claim(src: &[u16], symbols: &IndexMap<String, Origin>, item: &Item, kind: &str) -> Result<(), PeleError> {
    if let Some(old) = symbols.get(&item.id) {
        let label = item.generated.then_some(item.label.text.as_str());
        let ol = if old.generated { old.label.as_deref() } else { None };
        if old.kind != kind {
            return Err(conflict(
                src,
                format!("ID '{}' is declared as both {} and {kind}", item.id, old.kind),
                item.span,
                old.span,
                label,
                ol,
            ));
        }
        if old.generated || item.generated {
            return Err(conflict(
                src,
                format!("Generated ID '{}' collides with another declaration", item.id),
                item.span,
                old.span,
                label,
                ol,
            ));
        }
    }
    Ok(())
}
fn register(symbols: &mut IndexMap<String, Origin>, item: &Item, kind: &str) {
    symbols.insert(
        item.id.clone(),
        Origin {
            kind: kind.into(),
            span: item.span,
            generated: item.generated,
            label: Some(item.label.text.clone()),
        },
    );
}
fn unique(src: &[u16], symbols: &mut IndexMap<String, Origin>, id: &str, kind: &str, span: Span) -> Result<(), PeleError> {
    if let Some(prev) = symbols.get(id) {
        return Err(conflict(
            src,
            format!("ID '{id}' is declared more than once ({} and {kind})", prev.kind),
            span,
            prev.span,
            None,
            if prev.generated { prev.label.as_deref() } else { None },
        ));
    }
    symbols.insert(
        id.into(),
        Origin {
            kind: kind.into(),
            span,
            generated: false,
            label: None,
        },
    );
    Ok(())
}
fn metadata(src: &[u16], state: &mut State, props: &[Property], replace: bool) -> Result<(), PeleError> {
    for p in props {
        let id = &state.item.id;
        if state.kind == "actor" {
            let val = p.value.as_str();
            let previous = match p.key.as_str() {
                "type" if matches!(val, Some("normal" | "hollow" | "awesome")) => {
                    state.actor_type.as_ref().map(|s| serde_json::Value::String(s.clone()))
                }
                "icon" if val.is_some() => state.icon.as_ref().map(|s| serde_json::Value::String(s.clone())),
                "business" if p.value.is_boolean() => state.business.map(serde_json::Value::Bool),
                _ => return Err(invalid(src, id, "actor", p)),
            };
            if !replace && previous.as_ref().is_some_and(|v| *v != p.value) {
                return Err(conflict(
                    src,
                    format!("Actor '{id}' has conflicting {} metadata", p.key),
                    p.key_span,
                    state.item.span,
                    None,
                    None,
                ));
            }
            match p.key.as_str() {
                "type" => state.actor_type = val.map(str::to_string),
                "icon" => state.icon = val.map(str::to_string),
                _ => state.business = p.value.as_bool(),
            }
        } else if p.key == "business" && p.value.is_boolean() {
            let v = p.value.as_bool().unwrap();
            if !replace && state.business.is_some_and(|b| b != v) {
                return Err(conflict(
                    src,
                    format!("Use case '{id}' has conflicting business metadata"),
                    p.key_span,
                    state.item.span,
                    None,
                    None,
                ));
            }
            state.business = Some(v)
        } else {
            return Err(invalid(src, id, "usecase", p));
        }
    }
    Ok(())
}
fn infer(props: &[Property]) -> Option<String> {
    let mut possible = vec!["actor", "usecase", "boundary", "edge"];
    for p in props {
        match p.key.as_str() {
            "icon" => possible = vec!["actor"],
            "animate" | "animation" => possible = vec!["edge"],
            "type" => {
                possible = match p.value.as_str() {
                    Some("rect" | "package") => vec!["boundary"],
                    Some("normal" | "hollow" | "awesome") => vec!["actor"],
                    _ => Vec::new(),
                }
            }
            "business" => possible.retain(|k| *k != "boundary" && *k != "edge"),
            _ => return None,
        }
    }
    if possible.len() == 1 { Some(possible[0].into()) } else { None }
}
fn mention(first: &mut IndexMap<String, usize>, id: &str, at: usize) {
    first.entry(id.into()).and_modify(|p| *p = (*p).min(at)).or_insert(at);
}
fn endpoint(
    src: &[u16],
    end: &Endpoint,
    symbols: &mut IndexMap<String, Origin>,
    elements: &mut IndexMap<String, State>,
    first: &mut IndexMap<String, usize>,
) -> Result<String, PeleError> {
    let item = &end.item;
    if end.classed && !end.declares {
        return Err(fail(
            src,
            format!("Relationship endpoint '{}' uses ::: without declaring the node", item.id),
            item.span,
        ));
    }
    if let Some(origin) = symbols.get(&item.id) {
        return Ok(origin.kind.clone());
    }
    let mut item = item.clone();
    item.shape = Some("ellipse".into());
    item.classes.clear();
    elements.insert(
        item.id.clone(),
        State {
            item: item.clone(),
            kind: "usecase".into(),
            parent: None,
            actor_type: None,
            icon: None,
            business: None,
            stereotype_span: None,
        },
    );
    register(symbols, &item, "usecase");
    mention(first, &item.id, item.span.0);
    Ok("usecase".into())
}
fn stylable<'a>(
    model: &'a mut Model,
    src: &[u16],
    symbols: &IndexMap<String, Origin>,
    id: &str,
    span: Span,
) -> Result<(&'a mut Vec<String>, &'a mut Vec<String>), PeleError> {
    match symbols.get(id).map(|s| s.kind.as_str()) {
        Some("actor") => {
            if let Some(n) = model.actors.get_mut(id) {
                return Ok((&mut n.classes, &mut n.styles));
            }
        }
        Some("usecase") => {
            if let Some(n) = model.usecases.get_mut(id) {
                return Ok((&mut n.classes, &mut n.styles));
            }
        }
        Some("boundary") => {
            if let Some(n) = model.boundaries.get_mut(id) {
                return Ok((&mut n.classes, &mut n.styles));
            }
        }
        Some("json") => {
            if let Some(n) = model.jsons.get_mut(id) {
                return Ok((&mut n.classes, &mut n.styles));
            }
        }
        _ => {
            if let Some(n) = model.relationships.iter_mut().find(|n| n.id == id) {
                return Ok((&mut n.classes, &mut n.styles));
            }
        }
    }
    Err(fail(src, format!("Class/style target '{id}' is unresolved or anonymous"), span))
}
pub fn resolve(source: &str, d: Drafts, title: Option<&str>) -> Result<Model, PeleError> {
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut model = Model {
        direction: d.direction.clone().unwrap_or_else(|| "LR".into()),
        title: title.map(str::to_string),
        acc_title: d.acc_title.clone(),
        acc_descr: crate::diagrams::timeline::story::squeeze(&d.acc_descr),
        source: source.into(),
        ..Default::default()
    };
    let mut symbols: IndexMap<String, Origin> = IndexMap::new();
    let mut elements: IndexMap<String, State> = IndexMap::new();
    let mut boundaries: IndexMap<String, Group> = IndexMap::new();
    let mut first = IndexMap::new();
    let mut decl = Vec::new();
    for rel in &d.relations {
        mention(&mut first, &rel.source.item.id, rel.source.item.span.0);
        mention(&mut first, &rel.target.item.id, rel.target.item.span.0)
    }
    for (i, e) in d.elements.iter().enumerate() {
        mention(&mut first, &e.item.id, e.item.span.0);
        decl.push((e.item.span.0, 0, i))
    }
    for (i, g) in d.boundaries.iter().enumerate() {
        mention(&mut first, &g.id, g.span.0);
        decl.push((g.span.0, 1, i))
    }
    for (i, (j, span)) in d.jsons.iter().enumerate() {
        mention(&mut first, &j.id, span.0);
        decl.push((span.0, 2, i))
    }
    for (i, r) in d.relations.iter().enumerate() {
        if let (Some(id), Some(span)) = (&r.id, r.id_span)
            && !id.is_empty()
        {
            decl.push((span.0, 3, i))
        }
    }
    decl.sort_by_key(|x| x.0);
    for (_, kind, i) in decl {
        match kind {
            0 => {
                let draft = &d.elements[i];
                let item = &draft.item;
                claim(&src, &symbols, item, &draft.kind)?;
                let parent = draft.parent.clone().filter(|p| !p.id.is_empty());
                if let Some(old) = elements.get_mut(&item.id) {
                    if old.item.label.text != item.label.text || old.item.label.markdown != item.label.markdown {
                        return Err(conflict(
                            &src,
                            format!("ID '{}' has conflicting labels", item.id),
                            item.span,
                            old.item.span,
                            None,
                            None,
                        ));
                    }
                    if item.shape.is_some() && old.item.shape.is_some() && item.shape != old.item.shape {
                        return Err(conflict(
                            &src,
                            format!("Use case '{}' has conflicting shapes", item.id),
                            item.span,
                            old.item.span,
                            None,
                            None,
                        ));
                    }
                    if let (Some(parent), Some(prev)) = (parent.as_ref(), old.parent.as_ref().filter(|p| !p.id.is_empty()))
                        && parent.id != prev.id
                    {
                        return Err(conflict(
                            &src,
                            format!("Element '{}' belongs to more than one system boundary", item.id),
                            parent.span,
                            prev.span,
                            None,
                            None,
                        ));
                    }
                    if item.stereotype.as_ref().is_some_and(|s| !s.is_empty())
                        && old.item.stereotype.as_ref().is_some_and(|s| !s.is_empty())
                        && item.stereotype != old.item.stereotype
                    {
                        return Err(conflict(
                            &src,
                            format!("Element '{}' has conflicting stereotypes", item.id),
                            item.span,
                            old.stereotype_span.unwrap_or(old.item.span),
                            None,
                            None,
                        ));
                    }
                    old.item.shape = old.item.shape.take().or(item.shape.clone());
                    old.parent = old.parent.take().or(draft.parent.clone());
                    old.item.stereotype = old.item.stereotype.take().or(item.stereotype.clone());
                    old.stereotype_span = old
                        .stereotype_span
                        .or_else(|| item.stereotype.as_ref().filter(|s| !s.is_empty()).map(|_| item.span));
                    unique_push(&mut old.item.classes, &item.classes);
                    metadata(&src, old, item.metadata.as_deref().unwrap_or(&[]), false)?;
                } else {
                    let mut state = State {
                        item: item.clone(),
                        kind: draft.kind.clone(),
                        parent,
                        actor_type: None,
                        icon: None,
                        business: None,
                        stereotype_span: item.stereotype.as_ref().filter(|s| !s.is_empty()).map(|_| item.span),
                    };
                    metadata(&src, &mut state, item.metadata.as_deref().unwrap_or(&[]), false)?;
                    elements.insert(item.id.clone(), state);
                    register(&mut symbols, item, &draft.kind)
                }
            }
            1 => {
                let item = &d.boundaries[i];
                claim(&src, &symbols, item, "boundary")?;
                if let Some(old) = boundaries.get_mut(&item.id) {
                    if old.item.label.text != item.label.text || old.item.label.markdown != item.label.markdown {
                        return Err(conflict(
                            &src,
                            format!("Boundary '{}' has conflicting titles", item.id),
                            item.span,
                            old.item.span,
                            None,
                            None,
                        ));
                    }
                    unique_push(&mut old.item.classes, &item.classes)
                } else {
                    boundaries.insert(
                        item.id.clone(),
                        Group {
                            item: item.clone(),
                            kind: None,
                            members: Vec::new(),
                        },
                    );
                    register(&mut symbols, item, "boundary")
                }
            }
            2 => unique(&src, &mut symbols, &d.jsons[i].0.id, "json", d.jsons[i].1)?,
            _ => unique(
                &src,
                &mut symbols,
                d.relations[i].id.as_ref().unwrap(),
                "edge",
                d.relations[i].id_span.unwrap(),
            )?,
        }
    }
    let mut anonymous = 0;
    for r in &d.relations {
        let sk = endpoint(&src, &r.source, &mut symbols, &mut elements, &mut first)?;
        let tk = endpoint(&src, &r.target, &mut symbols, &mut elements, &mut first)?;
        let s = &r.source.item;
        let t = &r.target.item;
        if !matches!(sk.as_str(), "actor" | "usecase" | "json") {
            return Err(conflict(
                &src,
                format!("Relationship source '{}' cannot be {sk}", s.id),
                s.span,
                symbols[&s.id].span,
                None,
                None,
            ));
        }
        if !matches!(tk.as_str(), "actor" | "usecase" | "json") {
            return Err(conflict(
                &src,
                format!("Relationship target '{}' cannot be {tk}", t.id),
                t.span,
                symbols[&t.id].span,
                None,
                None,
            ));
        }
        if matches!(r.kind.as_str(), "include" | "extend") && (sk != "usecase" || tk != "usecase") {
            return Err(fail(&src, format!("{} relationship requires use-case endpoints", r.kind), r.span));
        }
        if r.kind == "generalization" && (sk == "json" || sk != tk) {
            return Err(fail(
                &src,
                "Generalization requires actor-to-actor or use-case-to-use-case endpoints".into(),
                r.span,
            ));
        }
        if r.kind == "association" && (sk == "json" || tk == "json") && r.arrow > 2 {
            return Err(fail(
                &src,
                format!(
                    "JSON relationship '{}' to '{}' permits only point, reversed-point, or markerless solid association",
                    s.id, t.id
                ),
                r.span,
            ));
        }
        let id = r.id.clone().unwrap_or_else(|| {
            let id = format!("edge-{anonymous}");
            anonymous += 1;
            id
        });
        model.relationships.push(Relationship {
            id,
            explicit_id: r.id.as_ref().is_some_and(|s| !s.is_empty()),
            source: s.id.clone(),
            target: t.id.clone(),
            kind: r.kind.clone(),
            arrow: r.arrow,
            label: r.label.clone(),
            minlen: r.minlen,
            ..Default::default()
        })
    }
    for (target, span, properties) in &d.metadata {
        let Some(origin) = symbols.get(target) else {
            let inferred = infer(properties).map(|k| format!(" (metadata implies {k})")).unwrap_or_default();
            return Err(fail(&src, format!("Metadata target '{target}' is unresolved{inferred}"), *span));
        };
        let kind = origin.kind.as_str();
        if kind == "actor" || kind == "usecase" {
            metadata(&src, elements.get_mut(target).unwrap(), properties, true)?;
            continue;
        }
        for p in properties {
            if kind == "boundary" && p.key == "type" && matches!(p.value.as_str(), Some("rect" | "package")) {
                boundaries.get_mut(target).unwrap().kind = p.value.as_str().map(str::to_string)
            } else if kind == "edge" && p.key == "animate" && p.value.is_boolean() {
                model.relationships.iter_mut().rev().find(|r| r.id == *target).unwrap().animate = p.value.as_bool().unwrap()
            } else if kind == "edge" && p.key == "animation" && matches!(p.value.as_str(), Some("fast" | "slow")) {
                let edge = model.relationships.iter_mut().rev().find(|r| r.id == *target).unwrap();
                edge.animation = p.value.as_str().map(str::to_string);
                edge.animate = true
            } else {
                return Err(invalid(&src, target, kind, p));
            }
        }
    }
    for e in &mut model.relationships {
        if e.animation.is_some() {
            e.animate = true
        }
    }
    for state in elements.values() {
        let item = &state.item;
        let icon = state.icon.as_ref().filter(|s| !s.is_empty());
        if state.kind == "actor" {
            if icon.is_some() && state.actor_type.as_ref().is_some_and(|s| s != "normal") {
                return Err(fail(
                    &src,
                    format!(
                        "Actor '{}' cannot combine icon with type '{}'",
                        item.id,
                        state.actor_type.as_ref().unwrap()
                    ),
                    item.span,
                ));
            }
            if state.business == Some(true) && (icon.is_some() || state.actor_type.as_deref() == Some("awesome")) {
                return Err(fail(
                    &src,
                    format!("Business actor '{}' must use normal or hollow geometry", item.id),
                    item.span,
                ));
            }
        } else if item.shape.as_deref() == Some("rect") && state.business == Some(true) {
            return Err(fail(
                &src,
                format!("Rectangular use case '{}' cannot be a business use case", item.id),
                item.span,
            ));
        }
    }
    let mut ordered: Vec<_> = elements.values().collect();
    ordered.sort_by_key(|s| first.get(&s.item.id).copied().unwrap_or(0));
    for s in ordered {
        let item = &s.item;
        let element = Element {
            id: item.id.clone(),
            label: item.label.text.clone(),
            markdown: item.label.markdown,
            kind: s.kind.clone(),
            actor_type: if s.icon.as_ref().is_some_and(|s| !s.is_empty()) {
                "icon".into()
            } else {
                s.actor_type.clone().unwrap_or_else(|| "normal".into())
            },
            shape: item.shape.clone().unwrap_or_else(|| "ellipse".into()),
            icon: s.icon.clone().filter(|s| !s.is_empty()),
            business: s.business.unwrap_or(false),
            stereotype: item.stereotype.clone().filter(|s| !s.is_empty()),
            parent: s.parent.as_ref().map(|p| p.id.clone()).filter(|s| !s.is_empty()),
            classes: item.classes.clone(),
            styles: Vec::new(),
        };
        if s.kind == "actor" {
            model.actors.insert(item.id.clone(), element);
        } else {
            model.usecases.insert(item.id.clone(), element);
        }
    }
    let mut declarations: Vec<_> = d.elements.iter().collect();
    declarations.sort_by_key(|d| d.item.span.0);
    for d in declarations {
        if let Some(parent) = d.parent.as_ref().filter(|p| !p.id.is_empty()) {
            let members = &mut boundaries.get_mut(&parent.id).unwrap().members;
            if !members.contains(&d.item.id) {
                members.push(d.item.id.clone())
            }
        }
    }
    let mut groups: Vec<_> = boundaries.into_values().collect();
    groups.sort_by_key(|s| first.get(&s.item.id).copied().unwrap_or(0));
    for s in groups {
        let item = s.item;
        model.boundaries.insert(
            item.id.clone(),
            Boundary {
                id: item.id,
                label: item.label.text,
                markdown: item.label.markdown,
                kind: s.kind.unwrap_or_else(|| "rect".into()),
                members: s.members,
                classes: item.classes,
                styles: Vec::new(),
            },
        );
    }
    let mut jsons = d.jsons;
    jsons.sort_by_key(|(j, _)| first.get(&j.id).copied().unwrap_or(0));
    for (j, _) in jsons {
        model.jsons.insert(j.id.clone(), j);
    }
    for (target, span, label) in d.notes {
        let Some(origin) = symbols.get(&target) else {
            return Err(fail(&src, format!("Note target '{target}' is unresolved"), span));
        };
        if !matches!(origin.kind.as_str(), "actor" | "usecase") {
            return Err(conflict(
                &src,
                format!("Note target '{target}' must be an actor or use case, not {}", origin.kind),
                span,
                origin.span,
                None,
                None,
            ));
        }
        model.notes.push(Note {
            id: format!("note-{}", model.notes.len()),
            target,
            label,
        })
    }
    for (ids, styles) in d.class_defs {
        for id in ids {
            model.class_defs.insert(id, styles.clone());
        }
    }
    for (targets, classes) in d.classes {
        for (id, span) in targets {
            let (c, _) = stylable(&mut model, &src, &symbols, &id, span)?;
            unique_push(c, &classes)
        }
    }
    for (id, span, styles) in d.styles {
        let (_, s) = stylable(&mut model, &src, &symbols, &id, span)?;
        s.extend(styles)
    }
    Ok(model)
}
