use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeleErrorCode {
    UnsupportedDiagram,
    Syntax,
    Semantic,
    Limit,
}

impl PeleErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            PeleErrorCode::UnsupportedDiagram => "unsupported-diagram",
            PeleErrorCode::Syntax => "syntax",
            PeleErrorCode::Semantic => "semantic",
            PeleErrorCode::Limit => "limit",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PeleError {
    pub message: String,
    pub code: PeleErrorCode,
    pub diagram: Option<String>,
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

impl PeleError {
    pub fn new(message: impl Into<String>, code: PeleErrorCode) -> Self {
        PeleError {
            message: message.into(),
            code,
            diagram: None,
            line: 0,
            column: 0,
            snippet: String::new(),
        }
    }

    pub fn of(mut self, diagram: &str) -> Self {
        self.diagram = Some(diagram.to_string());
        self
    }
}

impl fmt::Display for PeleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PeleError {}

pub type Result<T> = std::result::Result<T, PeleError>;

pub struct Location {
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

// Line, column and a caret under the offending character. Offsets count UTF-16 code units, as
// the lexers do. Unlike pele, positions are reported in the text after preprocessing.
pub fn locate(src: &[u16], offset: usize) -> Location {
    let mut end = src.len();
    while end > 0 && crate::util::is_space16(src[end - 1] as u32) {
        end -= 1;
    }
    let offset = if end > 0 && offset >= end { end } else { offset.min(src.len()) };
    let mut line = 1;
    let mut line_start = 0;
    for (i, &c) in src.iter().enumerate() {
        if i >= offset {
            break;
        }
        if c == 10 {
            line += 1;
            line_start = i + 1;
        }
    }
    let mut line_end = offset;
    while line_end < src.len() && src[line_end] != 10 {
        line_end += 1;
    }
    let column = offset - line_start + 1;
    let text = String::from_utf16_lossy(&src[line_start..line_end]);
    Location {
        line,
        column,
        snippet: format!("{}\n{}^", text, "-".repeat(column - 1)),
    }
}

pub fn syntax_error(diagram: &str, src: &[u16], offset: usize, detail: &str, lexical: bool) -> PeleError {
    let loc = locate(src, offset);
    let message = if lexical {
        format!("Lexical error on line {}. Unrecognized text.\n{}", loc.line, loc.snippet)
    } else {
        format!("Parse error on line {}:\n{}\n{}", loc.line, loc.snippet, detail)
    };
    PeleError {
        message,
        code: PeleErrorCode::Syntax,
        diagram: Some(diagram.to_string()),
        line: loc.line,
        column: loc.column,
        snippet: loc.snippet,
    }
}
