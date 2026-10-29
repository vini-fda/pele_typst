mod parse;
mod render;
pub use parse::parse;
pub use render::render;
#[derive(Clone, Debug)]
pub enum Node {
    Terminal(String),
    Nonterminal(String),
    Special(String),
    Sequence(Vec<usize>),
    Choice(Vec<usize>),
    Optional(usize),
    Repetition { element: usize, min: f64, max: f64 },
}
impl Node {
    pub fn parts(&self) -> &[usize] {
        match self {
            Self::Sequence(v) | Self::Choice(v) => v,
            Self::Optional(i) | Self::Repetition { element: i, .. } => std::slice::from_ref(i),
            _ => &[],
        }
    }
}
#[derive(Clone, Debug)]
pub struct Rule {
    pub name: String,
    pub definition: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub notation: String,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub rules: Vec<Rule>,
    pub nodes: Vec<Node>,
}
impl Model {
    fn add(&mut self, node: Node) -> usize {
        let i = self.nodes.len();
        self.nodes.push(node);
        i
    }
    fn sequence(&mut self, v: Vec<usize>) -> usize {
        if v.len() == 1 { v[0] } else { self.add(Node::Sequence(v)) }
    }
    fn choice(&mut self, v: Vec<usize>) -> usize {
        if v.len() == 1 { v[0] } else { self.add(Node::Choice(v)) }
    }
    fn repeat(&mut self, element: usize, min: f64, max: f64) -> usize {
        self.add(Node::Repetition { element, min, max })
    }
}
