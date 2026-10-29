use super::tokens as t;
fn is_space16(c: u16) -> bool {
    crate::util::is_space16(c as u32)
}
fn is_word16(c: u16) -> bool {
    crate::util::is_word16(c as u32)
}
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: usize,
    pub start: usize,
    pub text: String,
}
struct Lexer {
    src: Vec<u16>,
    p: usize,
    out: Vec<Token>,
}
impl Lexer {
    fn at(&self, p: usize) -> u16 {
        self.src.get(p).copied().unwrap_or(0)
    }
    fn starts(&self, p: usize, s: &str) -> bool {
        self.src
            .get(p..)
            .is_some_and(|v| v.starts_with(&s.encode_utf16().collect::<Vec<_>>()))
    }
    fn up_to(&self, c: u16, p: usize) -> usize {
        self.src[p.min(self.src.len())..]
            .iter()
            .position(|v| *v == c)
            .map_or(self.src.len(), |i| i + p)
    }
    fn ws(&self, mut p: usize) -> usize {
        while p < self.src.len() && is_space16(self.at(p)) {
            p += 1;
        }
        p
    }
    fn emit(&mut self, k: usize, s: usize, e: usize) {
        self.out.push(Token {
            kind: k,
            start: s,
            text: String::from_utf16_lossy(&self.src[s..e]),
        });
        self.p = e;
    }
    fn statement(&mut self, word: &str, k: usize) -> bool {
        let mut q = self.p + word.len();
        if !self.starts(self.p, word) || !is_space16(self.at(q)) {
            return false;
        }
        q += 1;
        let from = q;
        while q < self.src.len() && !matches!(self.at(q), 35 | 10 | 59) {
            q += 1;
        }
        if q == from {
            return false;
        }
        self.emit(k, self.p, q);
        true
    }
}
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut l = Lexer {
        src: src.encode_utf16().collect(),
        p: 0,
        out: vec![],
    };
    let n = l.src.len();
    let (mut state, mut acc, mut done) = (0, t::acc_title_value, false);
    let mut direction_failed_until = 0;
    loop {
        if done {
            l.emit(t::END, n, n);
            break;
        }
        if l.p >= n {
            done = true;
        }
        let p = l.p;
        let c = l.at(p);
        match state {
            0 => {
                if p >= n {
                    l.emit(t::EOF, n, n);
                    continue;
                }
                if p >= direction_failed_until {
                    let line = l.src[p..]
                        .iter()
                        .position(|v| matches!(*v, 10 | 13 | 0x2028 | 0x2029))
                        .map_or(n, |i| p + i);
                    let mut direction = None;
                    for (i, code) in ["TB", "BT", "RL", "LR"].iter().enumerate() {
                        let mut q = p;
                        while q < line {
                            if l.starts(q, "direction") {
                                let after = q + 9;
                                if is_space16(l.at(after)) {
                                    let r = l.ws(after);
                                    if l.starts(r, code) {
                                        direction = Some(t::direction_tb + i);
                                        break;
                                    }
                                }
                            }
                            q += 1;
                        }
                        if direction.is_some() {
                            break;
                        }
                    }
                    if let Some(k) = direction {
                        let end = l.up_to(10, p);
                        l.emit(k, p, end);
                        continue;
                    }
                    direction_failed_until = line;
                }
                if c == 116 {
                    if l.starts(p, "title") {
                        let mut q = p + 5;
                        while q < n && is_space16(l.at(q)) && !matches!(l.at(q), 10 | 13) {
                            q += 1;
                        }
                        if q == n || matches!(l.at(q), 10 | 13) {
                            l.p = l.ws(q);
                            continue;
                        }
                    }
                    if l.statement("title", t::title) {
                        continue;
                    }
                }
                if c == 97 {
                    if l.statement("accDescription", t::accDescription) {
                        continue;
                    }
                    let title = l.starts(p, "accTitle");
                    if title || l.starts(p, "accDescr") {
                        let q = l.ws(p + 8);
                        let d = l.at(q);
                        if d == 58 {
                            l.emit(if title { t::acc_title } else { t::acc_descr }, p, l.ws(q + 1));
                            acc = if title { t::acc_title_value } else { t::acc_descr_value };
                            state = 7;
                            continue;
                        }
                        if d == 123 && !title {
                            l.p = l.ws(q + 1);
                            state = 8;
                            continue;
                        }
                    }
                }
                if c == 37 && l.at(p + 1) == 37 {
                    let mut q = l.up_to(10, p + 2);
                    while q < n && matches!(l.at(q), 10 | 13) {
                        q += 1;
                    }
                    l.p = q;
                    continue;
                }
                if is_space16(c) {
                    let mut q = p;
                    let mut last = None;
                    while q < n && is_space16(l.at(q)) {
                        if l.at(q) == 10 {
                            last = Some(q);
                        }
                        q += 1;
                    }
                    if let Some(last) = last {
                        l.emit(t::NEWLINE, p, last + 1)
                    } else {
                        l.p = q
                    }
                    continue;
                }
                if c == 123 || c == 125 {
                    l.emit(if c == 123 { t::LBRACE } else { t::RBRACE }, p, p + 1);
                    continue;
                }
                if is_word16(c) {
                    let mut q = p + 1;
                    while q < n && is_word16(l.at(q)) {
                        q += 1;
                    }
                    if q - p <= 19
                        && let Some(k) = keyword(&String::from_utf16_lossy(&l.src[p..q]))
                    {
                        if k > t::C4_DEPLOYMENT {
                            state = 1;
                        }
                        l.emit(k, p, q);
                        continue;
                    }
                }
                l.emit(t::ERROR, p, p);
                break;
            }
            1 => {
                if p >= n {
                    l.emit(t::EOF_IN_STRUCT, n, n);
                } else if c == 40 {
                    let mut q = p + 1;
                    while l.at(q) == 32 {
                        q += 1;
                    }
                    if l.at(q) == 44 {
                        l.emit(t::ATTRIBUTE_EMPTY, p, q + 1);
                    } else {
                        l.p += 1;
                    }
                    state = 2;
                } else if c == 41 {
                    l.p += 1;
                    state = 0;
                } else {
                    l.emit(t::ERROR, p, p);
                    break;
                }
            }
            2 => {
                if p >= n {
                    continue;
                }
                if c == 41 {
                    l.p += 1;
                    state = 0;
                } else if c == 44 {
                    if l.at(p + 1) == 44 {
                        l.emit(t::ATTRIBUTE_EMPTY, p, p + 2);
                    } else {
                        l.p += 1;
                    }
                } else {
                    let mut q = p;
                    while l.at(q) == 32 {
                        q += 1;
                    }
                    match l.at(q) {
                        34 => {
                            if l.at(q + 1) == 34 {
                                l.emit(t::ATTRIBUTE_EMPTY, p, q + 2);
                            } else {
                                l.p = q + 1;
                                state = 3;
                            }
                        }
                        36 => {
                            l.p = q + 1;
                            state = 4;
                        }
                        _ => {
                            let end = l.up_to(44, p);
                            l.emit(t::STR, p, end);
                        }
                    }
                }
            }
            3 => {
                if c == 34 {
                    l.p += 1;
                    state = 2;
                } else {
                    let end = l.up_to(34, p);
                    l.emit(t::STR, p, end);
                }
            }
            4 => {
                let end = l.up_to(61, p);
                l.emit(t::STR_KEY, p, end);
                state = 5;
            }
            5 => {
                if p >= n {
                    continue;
                }
                let mut q = p + 1;
                if c == 61 {
                    while l.at(q) == 32 {
                        q += 1;
                    }
                }
                if c != 61 || l.at(q) != 34 {
                    l.emit(t::ERROR, p, p);
                    break;
                }
                l.p = q + 1;
                state = 6;
            }
            6 => {
                if p >= n {
                    continue;
                }
                if c == 34 {
                    l.p += 1;
                    state = 2;
                } else {
                    let end = l.up_to(34, p);
                    l.emit(t::STR_VALUE, p, end);
                }
            }
            7 => {
                let end = l.up_to(10, p);
                l.emit(acc, p, end);
                state = 0;
            }
            _ => {
                if c == 125 {
                    l.p += 1;
                    state = 0;
                } else {
                    let end = l.up_to(125, p);
                    l.emit(t::acc_descr_multiline_value, p, end);
                }
            }
        }
    }
    l.out
}
fn keyword(s: &str) -> Option<usize> {
    Some(match s {
        "C4Context" => t::C4_CONTEXT,
        "C4Container" => t::C4_CONTAINER,
        "C4Component" => t::C4_COMPONENT,
        "C4Dynamic" => t::C4_DYNAMIC,
        "C4Deployment" => t::C4_DEPLOYMENT,
        "Person" => t::PERSON,
        "Person_Ext" => t::PERSON_EXT,
        "System" => t::SYSTEM,
        "SystemDb" => t::SYSTEM_DB,
        "SystemQueue" => t::SYSTEM_QUEUE,
        "System_Ext" => t::SYSTEM_EXT,
        "SystemDb_Ext" => t::SYSTEM_EXT_DB,
        "SystemQueue_Ext" => t::SYSTEM_EXT_QUEUE,
        "Container" => t::CONTAINER,
        "ContainerDb" => t::CONTAINER_DB,
        "ContainerQueue" => t::CONTAINER_QUEUE,
        "Container_Ext" => t::CONTAINER_EXT,
        "ContainerDb_Ext" => t::CONTAINER_EXT_DB,
        "ContainerQueue_Ext" => t::CONTAINER_EXT_QUEUE,
        "Component" => t::COMPONENT,
        "ComponentDb" => t::COMPONENT_DB,
        "ComponentQueue" => t::COMPONENT_QUEUE,
        "Component_Ext" => t::COMPONENT_EXT,
        "ComponentDb_Ext" => t::COMPONENT_EXT_DB,
        "ComponentQueue_Ext" => t::COMPONENT_EXT_QUEUE,
        "Enterprise_Boundary" => t::ENTERPRISE_BOUNDARY,
        "System_Boundary" => t::SYSTEM_BOUNDARY,
        "Boundary" => t::BOUNDARY,
        "Container_Boundary" => t::CONTAINER_BOUNDARY,
        "Deployment_Node" => t::NODE,
        "Node" => t::NODE,
        "Node_L" => t::NODE_L,
        "Node_R" => t::NODE_R,
        "Rel" => t::REL,
        "BiRel" => t::BIREL,
        "Rel_Up" => t::REL_U,
        "Rel_U" => t::REL_U,
        "Rel_Down" => t::REL_D,
        "Rel_D" => t::REL_D,
        "Rel_Left" => t::REL_L,
        "Rel_L" => t::REL_L,
        "Rel_Right" => t::REL_R,
        "Rel_R" => t::REL_R,
        "Rel_Back" => t::REL_B,
        "RelIndex" => t::REL_INDEX,
        "UpdateElementStyle" => t::UPDATE_EL_STYLE,
        "UpdateRelStyle" => t::UPDATE_REL_STYLE,
        "UpdateLayoutConfig" => t::UPDATE_LAYOUT_CONFIG,
        _ => return None,
    })
}
