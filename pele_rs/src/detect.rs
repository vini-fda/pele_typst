use std::sync::OnceLock;

use regex::Regex;

// Mermaid's detectors, in the order Mermaid registers them. The first match wins.
const DETECTORS: &[(&str, &str, &str)] = &[
    ("flowchart", "flowchart-elk", r"^\s*flowchart-elk"),
    ("mindmap", "mindmap", r"^\s*mindmap"),
    ("architecture", "architecture", r"^\s*architecture"),
    ("agentflow", "agentflow", r"^\s*agentflow-beta\b"),
    ("c4", "c4", r"^\s*(?:C4Context|C4Container|C4Component|C4Dynamic|C4Deployment)"),
    ("kanban", "kanban", r"^\s*kanban"),
    ("class", "classDiagram", r"^\s*classDiagram"),
    ("er", "er", r"^\s*erDiagram"),
    ("gantt", "gantt", r"^\s*gantt"),
    ("info", "info", r"^\s*info"),
    ("pie", "pie", r"^\s*pie"),
    ("requirement", "requirement", r"^\s*requirement(Diagram)?"),
    ("sequence", "sequence", r"^\s*sequenceDiagram"),
    ("swimlane", "swimlane", r"^\s*swimlane-beta\b"),
    ("flowchart", "flowchart-v2", r"^\s*(graph|flowchart)"),
    ("timeline", "timeline", r"^\s*timeline"),
    ("gitGraph", "gitGraph", r"^\s*gitGraph"),
    ("state", "stateDiagram", r"^\s*stateDiagram"),
    ("journey", "journey", r"^\s*journey"),
    ("quadrantChart", "quadrantChart", r"^\s*quadrantChart"),
    ("sankey", "sankey", r"^\s*sankey(-beta)?"),
    ("packet", "packet", r"^\s*packet(-beta)?"),
    ("xychart", "xychart", r"^\s*xychart(-beta)?"),
    ("block", "block", r"^\s*block(-beta)?"),
    ("eventmodeling", "eventmodeling", r"^\s*eventmodeling"),
    ("treeView", "treeView", r"^\s*treeView-beta"),
    ("radar", "radar", r"^\s*radar-beta"),
    ("ishikawa", "ishikawa", r"(?i)^\s*ishikawa(-beta)?\b"),
    ("treemap", "treemap", r"^\s*treemap"),
    ("railroad", "railroad", r"(?i)^\s*railroad-beta"),
    ("railroad", "railroadEbnf", r"(?i)^\s*railroad-ebnf-beta"),
    ("railroad", "railroadAbnf", r"(?i)^\s*railroad-abnf-beta"),
    ("railroad", "railroadPeg", r"(?i)^\s*railroad-peg-beta"),
    ("venn", "venn", r"^\s*venn-beta"),
    ("wardley", "wardley", r"(?i)^\s*wardley-beta"),
    ("cynefin", "cynefin", r"^\s*cynefin-beta(?:[\s:]|$)"),
    ("usecase", "usecase", r"^\s*usecase-beta(?:\s|$)"),
];

pub struct Detected {
    pub kind: &'static str,
    pub mermaid_type: &'static str,
}

pub fn detect_diagram(cleaned: &str) -> Option<Detected> {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    // JavaScript's \b and \w only know ASCII.
    let res = RES.get_or_init(|| {
        DETECTORS
            .iter()
            .map(|d| {
                let pattern = if let Some(source) = d.2.strip_prefix("(?i)") {
                    // JS /i without /u does not fold long-s or Kelvin sign to
                    // ASCII header letters. Expand only the literal ASCII text.
                    let mut out = String::new();
                    let mut escaped = false;
                    for c in source.chars() {
                        if escaped {
                            out.push(c);
                            escaped = false;
                        } else if c == '\\' {
                            out.push(c);
                            escaped = true;
                        } else if c.is_ascii_alphabetic() {
                            out.push_str(&format!("[{}{}]", c.to_ascii_lowercase(), c.to_ascii_uppercase()));
                        } else {
                            out.push(c);
                        }
                    }
                    out
                } else {
                    d.2.to_string()
                };
                let pattern = pattern
                    .replace(r"[\s:]", &format!("[{}:]", crate::util::JS_WS))
                    .replace(r"\s", &format!("[{}]", crate::util::JS_WS))
                    .replace(r"\b", r"(?-u:\b)");
                Regex::new(&pattern).unwrap()
            })
            .collect()
    });
    for (i, (kind, mermaid, _)) in DETECTORS.iter().enumerate() {
        if res[i].is_match(cleaned) {
            return Some(Detected {
                kind,
                mermaid_type: mermaid,
            });
        }
    }
    None
}
