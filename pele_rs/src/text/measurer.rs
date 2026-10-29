use std::sync::OnceLock;

pub const BOLD: u32 = 1;
pub const ITALIC: u32 = 2;
pub const MONO: u32 = 4;
pub const SUB: u32 = 8;
pub const SUP: u32 = 16;
pub const UNDERLINE: u32 = 32;

// Approximate divisor for CSS font-size="smaller".
const SMALLER: f64 = 1.2;

pub trait TextMeasurer {
    fn width(&self, text: &str, size: f64, style: u32) -> f64;
}

// Advance widths of printable ASCII in thousandths of an em, two base-36 digits each, from Arial.
const REGULAR: &str = "7q7q9vfgfgopij5b9999atg87q997q7qfgfgfgfgfgfgfgfgfgfg7q7qg8g8g8fgs7ijijk2k2ijgzlmk27qdwijfgn5k2lmijlmk2ijgzk2ijq8ijijgz7q7q7qd1fg99fgfgdwfgfg7qfgfg6666dw66n5fgfgfgfg99dw7qfgdwk2dwdwdw9a789ag8";
const BOLD_TABLE: &str = "7q99d6fgfgopk26m9999atg87q997q7qfgfgfgfgfgfgfgfgfgfg9999g8g8g8gzr3k2k2k2k2ijgzlmk27qfgk2gzn5k2lmijlmk2ijgzk2ijq8ijijgz997q99g8fg99fggzfggzfg99gzgz7q7qfg7qopgzgzgzgzatfg99gzfglmfgfgdwat7satg8";

fn unpack(packed: &str) -> [u16; 95] {
    let mut out = [0u16; 95];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u16::from_str_radix(&packed[i * 2..i * 2 + 2], 36).unwrap();
    }
    out
}

fn regular() -> &'static [u16; 95] {
    static T: OnceLock<[u16; 95]> = OnceLock::new();
    T.get_or_init(|| unpack(REGULAR))
}

fn bold() -> &'static [u16; 95] {
    static T: OnceLock<[u16; 95]> = OnceLock::new();
    T.get_or_init(|| unpack(BOLD_TABLE))
}

fn is_wide(c: u32) -> bool {
    (0x1100..=0x115f).contains(&c)
        || (0x2e80..=0xa4cf).contains(&c)
        || (0xac00..=0xd7a3).contains(&c)
        || (0xf900..=0xfaff).contains(&c)
        || (0xfe30..=0xfe4f).contains(&c)
        || (0xff00..=0xff60).contains(&c)
        || (0xffe0..=0xffe6).contains(&c)
        || (0xd800..=0xdbff).contains(&c)
}

fn is_zero_width(c: u32) -> bool {
    c == 0xad
        || (0x300..=0x36f).contains(&c)
        || (0x200b..=0x200f).contains(&c)
        || (0xdc00..=0xdfff).contains(&c)
        || (0xfe00..=0xfe0f).contains(&c)
        || c == 0xfeff
}

// Estimates widths from a table of Arial advances, as pele does where no canvas is available.
pub struct MetricsMeasurer;

impl TextMeasurer for MetricsMeasurer {
    fn width(&self, text: &str, size: f64, style: u32) -> f64 {
        let mono = style & MONO != 0;
        let eff = if style & (SUB | SUP) != 0 { size / SMALLER } else { size };
        let table = if style & BOLD != 0 { bold() } else { regular() };
        let mut units: f64 = 0.0;
        for c in text.encode_utf16() {
            let c = c as u32;
            if c < 32 {
                continue;
            }
            if c < 127 {
                units += if mono { 600.0 } else { table[(c - 32) as usize] as f64 };
            } else if is_zero_width(c) {
                continue;
            } else if is_wide(c) {
                units += if mono { 1200.0 } else { 1000.0 };
            } else {
                units += if mono { 600.0 } else { 580.0 };
            }
        }
        (units * eff) / 1000.0
    }
}
