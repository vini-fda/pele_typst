// URL checks for click targets and images, after pele's util/url.ts.

use std::sync::OnceLock;

use regex::Regex;

use crate::RenderOptions;
use crate::svg::builder::{esc, is_invalid_xml};
use crate::text::entities::decode_entities;
use crate::util::{trim, trim_start};

const BLANK: &str = "about:blank";

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).unwrap())
}

fn re_ctrl() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, "[\u{0000}-\u{001F}\u{007F}-\u{009F}\u{2000}-\u{200D}\u{FEFF}\u{FFFE}\u{FFFF}]")
}
fn re_html_entities() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"&#([A-Za-z0-9_]+);?")
}
fn re_html_ctrl_entity() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)&(newline|tab);")
}
fn re_ws_escape() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(\\|%5[cC])((%(6[eE]|72|74))|[nrt])")
}
fn re_scheme() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?:^|[\r\n\u{2028}\u{2029}])[^\r\n\u{2028}\u{2029}]+(:|&colon;)")
}
fn re_invalid_protocol() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)(?:^|[\r\n\u{2028}\u{2029}])([^A-Za-z0-9_]*)(javascript|data|vbscript)")
}
fn re_scheme_name() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)^([a-z][a-z0-9+.-]*):")
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn decode(uri: &str) -> String {
    percent_decode(uri).unwrap_or_else(|| uri.to_string())
}

// Removes the control characters, entity codes and escapes that could hide a scheme.
fn unwrap(url: &str, percent: bool) -> Option<String> {
    let mut out = if percent { decode(trim(url)) } else { trim(url).to_string() };
    let mut rounds = 0;
    loop {
        rounds += 1;
        if rounds > 32 {
            return None;
        }
        out = re_ctrl().replace_all(&out, "").into_owned();
        out = re_html_entities()
            .replace_all(&out, |c: &regex::Captures| {
                c.get(1)
                    .and_then(|d| d.as_str().parse::<u32>().ok())
                    .and_then(|n| char::from_u32(n & 0xffff))
                    .map(String::from)
                    .unwrap_or_default()
            })
            .into_owned();
        out = re_html_ctrl_entity().replace_all(&out, "").into_owned();
        out = re_ctrl().replace_all(&out, "").into_owned();
        out = re_ws_escape().replace_all(&out, "").into_owned();
        out = trim(&out).to_string();
        if percent {
            out = decode(&out);
        }
        let more = re_ctrl().is_match(&out)
            || re_html_entities().is_match(&out)
            || re_html_ctrl_entity().is_match(&out)
            || re_ws_escape().is_match(&out);
        if !more {
            return Some(out);
        }
    }
}

// Use WHATWG URL parsing for host validation, IDNA, ports, dot segments and
// percent encoding, matching the browser/JavaScript URL constructor.
fn normalize_http(url: &str) -> Option<String> {
    ::url::Url::parse(url).ok().map(String::from)
}

// Refuses what @braintree/sanitize-url refuses, the check Mermaid applies to click targets.
pub fn sanitize_url(url: &str) -> String {
    if url.is_empty() {
        return BLANK.into();
    }
    let (Some(decoded), Some(kept)) = (unwrap(url, true), unwrap(url, false)) else {
        return BLANK.into();
    };
    if decoded.is_empty() || kept.is_empty() {
        return BLANK.into();
    }
    if decoded.starts_with('.') || decoded.starts_with('/') {
        return kept;
    }
    let start = trim_start(&decoded);
    let Some(scheme) = re_scheme().find(start) else { return kept };
    let protocol = trim(&scheme.as_str().to_lowercase()).to_string();
    if re_invalid_protocol().is_match(&protocol) {
        return BLANK.into();
    }
    let normalized = trim_start(&kept).replace('\\', "/");
    if protocol == "mailto:" || protocol.contains("://") {
        return normalized;
    }
    if protocol == "http:" || protocol == "https:" {
        return normalize_http(&normalized).unwrap_or_else(|| BLANK.into());
    }
    normalized
}

pub fn safe_url(url: &str) -> String {
    sanitize_url(&decode_entities(url))
}

const LINK_SCHEMES: [&str; 4] = ["http", "https", "mailto", "tel"];
const IMAGE_SCHEMES: [&str; 2] = ["http", "https"];

fn allowed(url: &str, schemes: &[String]) -> bool {
    let b = url.as_bytes();
    if b.len() >= 2 && (b[0] == b'/' || b[0] == b'\\') && (b[1] == b'/' || b[1] == b'\\') {
        return false;
    }
    match re_scheme_name().captures(url) {
        None => true,
        Some(c) => schemes.iter().any(|s| s == &c.get(1).unwrap().as_str().to_lowercase()),
    }
}

fn allow(url: &str, schemes: &[String]) -> Option<String> {
    let decoded = unwrap(url, true)?;
    if allowed(url, schemes) && allowed(&decoded, schemes) {
        Some(url.to_string())
    } else {
        None
    }
}

fn schemes(given: &Option<Vec<String>>, fallback: &[&str]) -> Vec<String> {
    given.clone().unwrap_or_else(|| fallback.iter().map(|s| s.to_string()).collect())
}

pub fn link_url(url: &str, options: &RenderOptions) -> Option<String> {
    if options.links == Some(false) {
        return None;
    }
    allow(url, &schemes(&options.link_schemes, &LINK_SCHEMES))
}

pub fn image_url(url: &str, options: &RenderOptions) -> Option<String> {
    if options.images == Some(false) {
        return None;
    }
    allow(url, &schemes(&options.image_schemes, &IMAGE_SCHEMES))
}

pub fn internal_link(name: &str, options: &RenderOptions) -> bool {
    let url = sanitize_url(name);
    if url == BLANK || link_url(&url, options).is_none() {
        return false;
    }
    let seen: String = name
        .chars()
        .filter(|&c| !is_invalid_xml(c) && !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let seen = seen.trim_start_matches(|c: char| (c as u32) <= 0x20);
    link_url(seen, options).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkInfo {
    pub id: String,
    pub href: String,
    pub internal: bool,
}

pub fn linked(body: &str, url: &str, id: &str, links: &mut Vec<LinkInfo>, options: &RenderOptions, attrs: &str) -> String {
    let Some(href) = link_url(url, options) else {
        return body.to_string();
    };
    links.push(LinkInfo {
        id: id.to_string(),
        href: href.clone(),
        internal: false,
    });
    format!("<a href=\"{}\"{}{}>{}</a>", esc(&href), attrs, rel_attr(options), body)
}

pub fn rel_attr(options: &RenderOptions) -> String {
    let rel = options.link_rel.as_deref().unwrap_or("noopener");
    if rel.is_empty() {
        String::new()
    } else {
        format!(" rel=\"{}\"", esc(rel))
    }
}
