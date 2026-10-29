use std::cell::RefCell;
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Edit {
    pub from: usize,
    pub to: usize,
    pub length: usize,
}

struct Origin {
    parsed: Vec<u16>,
    source: String,
    keep_comments: bool,
    steps: Option<Vec<Vec<Edit>>>,
}
thread_local! { static ORIGINS: RefCell<Vec<Origin>> = const { RefCell::new(Vec::new()) }; }

pub(crate) fn locating_in<T>(parsed: &str, source: &str, keep_comments: bool, run: impl FnOnce() -> T) -> T {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            ORIGINS.with(|origins| {
                origins.borrow_mut().pop();
            });
        }
    }
    ORIGINS.with(|origins| {
        origins.borrow_mut().push(Origin {
            parsed: parsed.encode_utf16().collect(),
            source: source.into(),
            keep_comments,
            steps: None,
        })
    });
    let _guard = Guard;
    run()
}

pub(crate) fn written(src: &[u16], at: usize) -> (Vec<u16>, usize) {
    ORIGINS.with(|origins| {
        let mut origins = origins.borrow_mut();
        let Some(origin) = origins.last_mut().filter(|o| o.parsed == src) else {
            return (src.to_vec(), at);
        };
        if origin.steps.is_none() {
            origin.steps =
                Some(crate::preprocess::source_edits(&origin.source, origin.keep_comments).expect("preprocessing already succeeded"));
        }
        let mut at = at;
        for edits in origin.steps.as_ref().unwrap().iter().rev() {
            let mut shift: isize = 0;
            let mut mapped = None;
            for edit in edits {
                let start = edit.from as isize - shift;
                if (at as isize) < start {
                    break;
                }
                if (at as isize) < start + edit.length as isize {
                    mapped = Some(edit.from + ((at as isize - start) as usize).min(edit.to - edit.from - 1));
                    break;
                }
                shift += (edit.to - edit.from) as isize - edit.length as isize;
            }
            at = mapped.unwrap_or_else(|| (at as isize + shift).max(0) as usize);
        }
        (
            (crate::preprocess::normalize_line_breaks(&origin.source) + "\n")
                .encode_utf16()
                .collect(),
            at,
        )
    })
}

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
    /// Exact JavaScript UTF-16 message when it contains unpaired surrogates.
    /// `message` always contains the corresponding well-formed Unicode text.
    // Rare JS-only diagnostics are boxed so ordinary Result errors stay compact.
    #[allow(clippy::box_collection)]
    pub message_utf16: Option<Box<Vec<u16>>>,
    pub code: PeleErrorCode,
    pub diagram: Option<String>,
    pub mermaid_type: Option<String>,
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

impl PeleError {
    pub fn new(message: impl Into<String>, code: PeleErrorCode) -> Self {
        PeleError {
            message: message.into(),
            message_utf16: None,
            code,
            diagram: None,
            mermaid_type: None,
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
// the lexers do. A scoped origin translates parser offsets back to the written source.
pub fn locate(src: &[u16], offset: usize) -> Location {
    let mut end = src.len();
    while end > 0 && crate::util::is_space16(src[end - 1] as u32) {
        end -= 1;
    }
    let (source, offset) = if end > 0 && offset >= end {
        let (source, at) = written(src, end - 1);
        (source, at + 1)
    } else {
        written(src, offset.min(src.len()))
    };
    let src = source.as_slice();
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
        message_utf16: None,
        code: PeleErrorCode::Syntax,
        diagram: Some(diagram.to_string()),
        mermaid_type: None,
        line: loc.line,
        column: loc.column,
        snippet: loc.snippet,
    }
}
