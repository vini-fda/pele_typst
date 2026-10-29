use crate::{PeleError, errors::PeleErrorCode};
use indexmap::IndexMap;
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub id: String,
    pub junction: bool,
    pub icon: Option<String>,
    pub icon_text: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Group {
    pub id: String,
    pub icon: Option<String>,
    pub title: Option<String>,
    pub parent: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Edge {
    pub lhs_id: String,
    pub lhs_dir: usize,
    pub lhs_into: bool,
    pub lhs_group: bool,
    pub rhs_id: String,
    pub rhs_dir: usize,
    pub rhs_into: bool,
    pub rhs_group: bool,
    pub title: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Hint {
    pub direction: String,
    pub members: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub nodes: IndexMap<String, Node>,
    pub groups: IndexMap<String, Group>,
    pub edges: Vec<Edge>,
    pub hints: Vec<Hint>,
}
fn fail(message: String) -> PeleError {
    PeleError::new(message, PeleErrorCode::Semantic).of("architecture")
}
impl Model {
    fn register(&self, kind: &str, id: &str, parent: &Option<String>) -> Result<(), PeleError> {
        let prior = if self.groups.contains_key(id) {
            Some("group")
        } else if self.nodes.contains_key(id) {
            Some("node")
        } else {
            None
        };
        if let Some(prior) = prior {
            return Err(fail(format!("The {kind} id [{id}] is already in use by another {prior}")));
        }
        if let Some(p) = parent {
            if id == p {
                return Err(fail(format!("The {kind} [{id}] cannot be placed within itself")));
            }
            if !self.groups.contains_key(p) && !self.nodes.contains_key(p) {
                return Err(fail(format!(
                    "The {kind} [{id}]'s parent does not exist. Please make sure the parent is created before this {kind}"
                )));
            }
            if self.nodes.contains_key(p) {
                return Err(fail(format!("The {kind} [{id}]'s parent is not a group")));
            }
        }
        Ok(())
    }
    pub fn add_group(&mut self, g: Group) -> Result<(), PeleError> {
        self.register("group", &g.id, &g.parent)?;
        self.groups.insert(g.id.clone(), g);
        Ok(())
    }
    pub fn add_node(&mut self, n: Node) -> Result<(), PeleError> {
        self.register(if n.junction { "junction" } else { "service" }, &n.id, &n.parent)?;
        self.nodes.insert(n.id.clone(), n);
        Ok(())
    }
    pub fn add_edge(&mut self, e: Edge) -> Result<(), PeleError> {
        for (side, id) in [("left", &e.lhs_id), ("right", &e.rhs_id)] {
            if !self.nodes.contains_key(id) && !self.groups.contains_key(id) {
                return Err(fail(format!(
                    "The {side}-hand id [{id}] does not yet exist. Please create the service/group before declaring an edge to it."
                )));
            }
        }
        for (side, id) in [("left", &e.lhs_id), ("right", &e.rhs_id)] {
            if !self.nodes.contains_key(id) {
                return Err(fail(format!(
                    "The {side}-hand id [{id}] is a group. Add {{group}} to a service inside it instead."
                )));
            }
        }
        let parent = &self.nodes[&e.lhs_id].parent;
        if parent.is_some() && parent == &self.nodes[&e.rhs_id].parent {
            for (side, id, crosses) in [("left", &e.lhs_id, e.lhs_group), ("right", &e.rhs_id, e.rhs_group)] {
                if crosses {
                    return Err(fail(format!(
                        "The {side}-hand id [{id}] is modified to traverse the group boundary, but the edge does not pass through two groups."
                    )));
                }
            }
        }
        self.edges.push(e);
        Ok(())
    }
    pub fn add_hint(&mut self, h: Hint) -> Result<(), PeleError> {
        if h.members.len() < 2 {
            return Err(fail(format!(
                "An align directive requires at least two members; got {}",
                h.members.len()
            )));
        }
        let mut seen = std::collections::HashSet::new();
        for id in &h.members {
            if !self.nodes.contains_key(id) {
                return Err(fail(format!(
                    "align {} references [{id}], which is not a service or junction",
                    h.direction
                )));
            }
            if !seen.insert(id) {
                return Err(fail(format!("align {} lists [{id}] more than once", h.direction)));
            }
        }
        self.hints.push(h);
        Ok(())
    }
}
