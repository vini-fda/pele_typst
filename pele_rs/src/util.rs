// Helpers that reproduce JavaScript's number, string and sort semantics, so that the SVG this
// crate writes matches pele's byte for byte.

use std::cmp::Ordering;

// Math.round: halves round towards +Infinity.
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

// Math.sign, with NaN and signed zeros passed through.
pub fn js_sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

// Math.min / Math.max over two numbers: NaN wins, and -0 is less than +0.
pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b || (a == b && b.is_sign_negative()) {
        a
    } else {
        b
    }
}

// Number.prototype.toString().
pub fn js_num(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let abs = x.abs();
    if abs >= 1e21 || abs < 1e-6 {
        // Rust writes 1.5e-7 and 1e21; JavaScript writes 1.5e-7 and 1e+21.
        let s = format!("{:e}", x);
        if let Some(at) = s.find('e') {
            let (mant, exp) = s.split_at(at);
            let exp = &exp[1..];
            if exp.starts_with('-') {
                return format!("{}e{}", mant, exp);
            }
            return format!("{}e+{}", mant, exp);
        }
        return s;
    }
    if x.fract() == 0.0 {
        return format!("{:.0}", x);
    }
    format!("{}", x)
}

fn fraction(i: u64) -> String {
    if i == 0 {
        String::new()
    } else if i.is_multiple_of(10) {
        format!(".{}", i / 10)
    } else if i < 10 {
        format!(".0{}", i)
    } else {
        format!(".{}", i)
    }
}

// The two decimals of a coordinate, as pele writes them.
pub fn num(value: f64) -> String {
    let n = js_round(value * 100.0);
    if n == 0.0 {
        return "0".into();
    }
    if n > 0.0 {
        if n < 1e11 {
            let q = (n / 100.0).floor();
            let r = (n - q * 100.0) as u64;
            return format!("{}{}", q as u64, fraction(r));
        }
    } else if n > -1e11 {
        let m = -n;
        let q = (m / 100.0).floor();
        let r = (m - q * 100.0) as u64;
        return format!("-{}{}", q as u64, fraction(r));
    }
    js_num(n / 100.0)
}

// Whitespace as JavaScript's `\s` and String.prototype.trim() see it.
pub fn is_js_space(c: char) -> bool {
    let c = c as u32;
    if c < 128 {
        return c == 32 || (9..=13).contains(&c);
    }
    c == 0xa0
        || c == 0x1680
        || (0x2000..=0x200a).contains(&c)
        || c == 0x2028
        || c == 0x2029
        || c == 0x202f
        || c == 0x205f
        || c == 0x3000
        || c == 0xfeff
}

pub fn is_space16(c: u32) -> bool {
    if c < 128 {
        return c == 32 || (9..=13).contains(&c);
    }
    c == 0xa0
        || c == 0x1680
        || (0x2000..=0x200a).contains(&c)
        || c == 0x2028
        || c == 0x2029
        || c == 0x202f
        || c == 0x205f
        || c == 0x3000
        || c == 0xfeff
}

pub fn is_word16(c: u32) -> bool {
    (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c) || c == 95
}

pub fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// A character class for regexes, matching JavaScript's `\s`.
pub const JS_WS: &str = r"\t\n\x0B\x0C\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";

pub fn trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

pub fn trim_start(s: &str) -> &str {
    s.trim_start_matches(is_js_space)
}

pub fn trim_end(s: &str) -> &str {
    s.trim_end_matches(is_js_space)
}

// parseFloat(): the longest numeric prefix after leading whitespace.
pub fn parse_float(s: &str) -> f64 {
    let s = trim_start(s);
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    if s[i..].starts_with("Infinity") {
        return if b.first() == Some(&b'-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let digits_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - digits_start;
    if i < b.len() && b[i] == b'.' {
        let dot = i;
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - dot - 1;
        if digits == 0 {
            return f64::NAN;
        }
    }
    if digits == 0 {
        return f64::NAN;
    }
    let mut end = i;
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            end = j;
        }
    }
    s[..end].parse::<f64>().unwrap_or(f64::NAN)
}

// Number(): the whole string must be numeric, and an empty one is 0.
pub fn js_number(s: &str) -> f64 {
    let t = trim(s);
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).map(|v| v as f64).unwrap_or(f64::NAN);
    }
    let ok = t
        .bytes()
        .all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'));
    if !ok {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

// A comparator result as JavaScript's sort reads it: NaN and 0 both mean equal.
pub fn ord(d: f64) -> Ordering {
    if d < 0.0 {
        Ordering::Less
    } else if d > 0.0 {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

// A stable merge sort that, unlike the standard library's, accepts any comparator.
pub fn stable_sort_by<T: Clone, F: FnMut(&T, &T) -> Ordering>(list: &mut [T], mut cmp: F) {
    let n = list.len();
    if n < 2 {
        return;
    }
    // Insertion sort for small runs, then merges.
    const RUN: usize = 16;
    let mut s = 0;
    while s < n {
        let end = (s + RUN).min(n);
        for i in s + 1..end {
            let mut j = i;
            while j > s && cmp(&list[j], &list[j - 1]) == Ordering::Less {
                list.swap(j, j - 1);
                j -= 1;
            }
        }
        s = end;
    }
    let mut width = RUN;
    let mut buf: Vec<T> = list.to_vec();
    while width < n {
        let mut s = 0;
        while s < n {
            let mid = (s + width).min(n);
            let end = (s + 2 * width).min(n);
            let (mut i, mut j, mut k) = (s, mid, s);
            while i < mid && j < end {
                if cmp(&list[j], &list[i]) == Ordering::Less {
                    buf[k] = list[j].clone();
                    j += 1;
                } else {
                    buf[k] = list[i].clone();
                    i += 1;
                }
                k += 1;
            }
            while i < mid {
                buf[k] = list[i].clone();
                i += 1;
                k += 1;
            }
            while j < end {
                buf[k] = list[j].clone();
                j += 1;
                k += 1;
            }
            s = end;
        }
        list.clone_from_slice(&buf);
        width *= 2;
    }
}

// String.prototype.replace with a string pattern: the first occurrence only.
pub fn replace_first(s: &str, from: &str, to: &str) -> String {
    match s.find(from) {
        Some(at) => format!("{}{}{}", &s[..at], to, &s[at + from.len()..]),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(num(0.0), "0");
        assert_eq!(num(1.005), "1");
        assert_eq!(num(-2.5), "-2.5");
        assert_eq!(num(12.345), "12.35");
        assert_eq!(num(-0.004), "0");
        assert_eq!(num(3.1), "3.1");
        assert_eq!(num(3.07), "3.07");
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_num(0.1), "0.1");
        assert_eq!(js_num(301.0), "301");
        assert_eq!(js_num(1e21), "1e+21");
        assert_eq!(parse_float("12px"), 12.0);
        assert!(parse_float("px").is_nan());
    }
}
