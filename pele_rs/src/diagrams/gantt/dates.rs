//! The local-time dayjs operations used by Pele's Gantt implementation.
use crate::RenderOptions;
use crate::errors::{PeleError, PeleErrorCode};
use crate::util::{js_num, js_number, js_round};
use chrono::{Duration, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use regex::Regex;
use std::sync::OnceLock;
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
pub const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
pub const DAY: f64 = 86400000.0;
#[derive(Debug, Clone)]
pub enum Zone {
    Named(Tz),
    #[cfg(not(target_arch = "wasm32"))]
    Local,
}
#[derive(Debug, Clone, Copy)]
pub struct Parts {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub weekday: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub milliseconds: u32,
    pub offset: i32,
    pub ordinal: u32,
    pub iso_year: i32,
    pub iso_week: u32,
}
// Proleptic Gregorian conversion covers the entire ECMAScript Date range, which
// extends beyond chrono's NaiveDate range. Offset lookup uses the same 400-year cycle.
fn equivalent_year(year: i32) -> i32 {
    let first = civil_days(year as i64, 1, 1).rem_euclid(7);
    let leap = civil_days(year as i64 + 1, 1, 1) - civil_days(year as i64, 1, 1) == 366;
    (2008..2036)
        .find(|&y| civil_days(y, 1, 1).rem_euclid(7) == first && (civil_days(y + 1, 1, 1) - civil_days(y, 1, 1) == 366) == leap)
        .unwrap() as i32
}
fn civil_days(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let y = year - era * 400;
    let m = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * m + 2) / 5 + day - 1;
    era * 146097 + y * 365 + y / 4 - y / 100 + doy - 719468
}
fn civil_date(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    (y as i32, m as u32 - 1, d as u32)
}
fn utc_parts(time: f64) -> Parts {
    let millis = time.trunc() as i64;
    let days = millis.div_euclid(86400000);
    let rem = millis.rem_euclid(86400000);
    let (year, month, day) = civil_date(days);
    let weekday = (days + 4).rem_euclid(7) as u32;
    let iso_day = if weekday == 0 { 7 } else { weekday };
    let thursday = days + 4 - iso_day as i64;
    let iso_year = civil_date(thursday).0;
    let jan4 = civil_days(iso_year as i64, 1, 4);
    let first = jan4 - ((jan4 + 3).rem_euclid(7));
    Parts {
        year,
        month,
        day,
        weekday,
        hours: (rem / 3600000) as u32,
        minutes: (rem / 60000 % 60) as u32,
        seconds: (rem / 1000 % 60) as u32,
        milliseconds: (rem % 1000) as u32,
        offset: 0,
        ordinal: (days - civil_days(year as i64, 1, 1) + 1) as u32,
        iso_year,
        iso_week: ((days - first).div_euclid(7) + 1) as u32,
    }
}
impl Zone {
    pub fn new(name: Option<&str>) -> Result<Self, PeleError> {
        if let Some(name) = name {
            return name
                .parse()
                .map(Self::Named)
                .map_err(|_| PeleError::new(format!("Unknown time zone: {name}"), PeleErrorCode::Semantic).of("gantt"));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(name) = std::env::var("TZ")
                && let Ok(zone) = name.parse()
            {
                return Ok(Self::Named(zone));
            }
            Ok(Self::Local)
        }
        #[cfg(target_arch = "wasm32")]
        {
            Ok(Self::Named(chrono_tz::UTC))
        }
    }
    pub fn parts(&self, time: f64) -> Option<Parts> {
        if !valid(time) {
            return None;
        }
        let wall = utc_parts(time);
        let surrogate = if wall.year <= 2037 && Utc.timestamp_millis_opt(time.trunc() as i64).single().is_some() {
            time
        } else {
            let year = if wall.year > 0 {
                equivalent_year(wall.year)
            } else {
                -10400 + wall.year.rem_euclid(400)
            };
            civil_days(year as i64, wall.month as i64 + 1, wall.day as i64) as f64 * DAY + time.rem_euclid(DAY)
        };
        let utc = Utc.timestamp_millis_opt(surrogate.trunc() as i64).single()?;
        let seconds = match self {
            Self::Named(z) => utc.with_timezone(z).offset().fix().local_minus_utc(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Local => utc.with_timezone(&chrono::Local).offset().fix().local_minus_utc(),
        };
        let mut out = utc_parts(time + seconds as f64 * 1000.0);
        out.offset = seconds / 60;
        Some(out)
    }
    pub fn local(&self, date: NaiveDateTime) -> Option<f64> {
        fn resolve<T: TimeZone>(zone: &T, date: NaiveDateTime) -> Option<f64> {
            match zone.from_local_datetime(&date) {
                LocalResult::Single(d) => Some(d.timestamp_millis() as f64),
                LocalResult::Ambiguous(a, b) => Some(a.timestamp_millis().min(b.timestamp_millis()) as f64),
                LocalResult::None => {
                    for minutes in 1..=2880 {
                        let before = date.checked_sub_signed(Duration::minutes(minutes))?;
                        if let Some(d) = zone.from_local_datetime(&before).earliest() {
                            return Some(date.and_utc().timestamp_millis() as f64 - d.offset().fix().local_minus_utc() as f64 * 1000.0);
                        }
                    }
                    None
                }
            }
        }
        match self {
            Self::Named(z) => resolve(z, date),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Local => resolve(&chrono::Local, date),
        }
    }
}
pub fn clock(options: &RenderOptions) -> Result<f64, PeleError> {
    if let Some(now) = options.now.filter(|v| v.is_finite()) {
        return Ok(now);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(Utc::now().timestamp_millis() as f64)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err(PeleError::new(
            "Gantt diagrams require an explicit finite now timestamp in WebAssembly.",
            PeleErrorCode::Semantic,
        )
        .of("gantt"))
    }
}
pub fn valid(time: f64) -> bool {
    time.is_finite() && time.abs() <= 8.64e15
}
pub fn make(
    year: f64,
    month: f64,
    day: f64,
    hours: f64,
    minutes: f64,
    seconds: f64,
    ms: f64,
    zone: &Zone,
    constructor: bool,
) -> Option<f64> {
    if [year, month, day, hours, minutes, seconds, ms]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1e15)
    {
        return None;
    }
    let mut year = year.trunc() as i64;
    if constructor && (0..=99).contains(&year) {
        year += 1900;
    }
    let total = year.checked_mul(12)?.checked_add(month.trunc() as i64)?;
    let year = total.div_euclid(12);
    let month = total.rem_euclid(12) as u32 + 1;
    if year.abs() > 1_000_000 {
        return None;
    }
    let wall = civil_days(year, month as i64, 1) as f64 * DAY
        + (day.trunc() - 1.0) * DAY
        + hours.trunc() * 3600000.0
        + minutes.trunc() * 60000.0
        + seconds.trunc() * 1000.0
        + ms.trunc();
    if !wall.is_finite() || wall.abs() > 8.64e15 + 2.0 * DAY {
        return None;
    }
    if let Some(date) = Utc
        .timestamp_millis_opt(wall.trunc() as i64)
        .single()
        .filter(|_| utc_parts(wall).year <= 2037)
    {
        return zone.local(date.naive_utc()).filter(|&d| valid(d));
    }
    let p = utc_parts(wall);
    let year = if p.year > 0 {
        equivalent_year(p.year)
    } else {
        -10400 + p.year.rem_euclid(400)
    };
    let surrogate = NaiveDate::from_ymd_opt(year, p.month + 1, p.day)?.and_hms_milli_opt(p.hours, p.minutes, p.seconds, p.milliseconds)?;
    let local = zone.local(surrogate)?;
    let offset = local - surrogate.and_utc().timestamp_millis() as f64;
    let result = wall + offset;
    valid(result).then_some(result)
}
pub fn set(
    time: f64,
    zone: &Zone,
    year: Option<f64>,
    month: Option<f64>,
    day: Option<f64>,
    hours: Option<f64>,
    minutes: Option<f64>,
    seconds: Option<f64>,
    ms: Option<f64>,
) -> f64 {
    let p = match zone.parts(time) {
        Some(p) => p,
        None if year.is_some() => utc_parts(0.0),
        None => return f64::NAN,
    };
    make(
        year.unwrap_or(p.year as f64),
        month.unwrap_or(p.month as f64),
        day.unwrap_or(p.day as f64),
        hours.unwrap_or(p.hours as f64),
        minutes.unwrap_or(p.minutes as f64),
        seconds.unwrap_or(p.seconds as f64),
        ms.unwrap_or(p.milliseconds as f64),
        zone,
        false,
    )
    .unwrap_or(f64::NAN)
}
pub fn start_day(time: f64, zone: &Zone) -> f64 {
    set(time, zone, None, None, None, Some(0.0), Some(0.0), Some(0.0), Some(0.0))
}
pub fn add_days(time: f64, days: f64, zone: &Zone) -> f64 {
    let Some(p) = zone.parts(time) else {
        return f64::NAN;
    };
    set(time, zone, None, None, Some(p.day as f64 + days), None, None, None, None)
}
pub fn add(time: f64, amount: f64, unit: &str, zone: &Zone) -> f64 {
    match unit {
        "M" | "y" => {
            let Some(p) = zone.parts(time) else {
                return f64::NAN;
            };
            let first = set(time, zone, None, None, Some(1.0), None, None, None, None);
            let next = set(
                first,
                zone,
                if unit == "y" { Some(p.year as f64 + amount) } else { None },
                if unit == "M" { Some(p.month as f64 + amount) } else { None },
                None,
                None,
                None,
                None,
                None,
            );
            let Some(q) = zone.parts(next) else {
                return f64::NAN;
            };
            let Some(last) = make(q.year as f64, q.month as f64 + 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, zone, true)
                .map(|d| set(d, zone, None, None, None, Some(23.0), Some(59.0), Some(59.0), Some(999.0)))
                .and_then(|d| zone.parts(d))
                .map(|d| d.day)
            else {
                return f64::NAN;
            };
            set(next, zone, None, None, Some(p.day.min(last) as f64), None, None, None, None)
        }
        "d" => add_days(time, js_round(amount), zone),
        "w" => add_days(time, js_round(7.0 * amount), zone),
        _ => {
            let value = (time
                + amount
                    * match unit {
                        "h" => 3600000.0,
                        "m" => 60000.0,
                        "s" => 1000.0,
                        _ => 1.0,
                    })
            .trunc();
            if valid(value) { value } else { f64::NAN }
        }
    }
}
fn pad(value: i64, length: usize) -> String {
    let s = value.to_string();
    if s.len() >= length { s } else { "0".repeat(length - s.len()) + &s }
}
fn ordinal(n: u32) -> String {
    let suffix = if (11..=13).contains(&(n % 100)) {
        "th"
    } else {
        match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{n}{suffix}")
}
pub fn iso_date(time: f64, zone: &Zone) -> String {
    let Some(p) = zone.parts(time) else {
        return "Invalid Date".into();
    };
    format!("{}-{}-{}", pad(p.year as i64, 4), pad(p.month as i64 + 1, 2), pad(p.day as i64, 2))
}
fn iso_week(time: f64, zone: &Zone) -> f64 {
    let Some(p) = zone.parts(time) else {
        return f64::NAN;
    };
    let thursday = add_days(time, 4.0 - if p.weekday == 0 { 7.0 } else { p.weekday as f64 }, zone);
    let Some(t) = zone.parts(thursday) else {
        return f64::NAN;
    };
    let Some(jan) = make(t.year as f64, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, zone, true) else {
        return f64::NAN;
    };
    let day = zone.parts(jan).unwrap().weekday;
    let day = if day == 0 { 7 } else { day };
    let first = add_days(jan, 4.0 - day as f64 + if day > 4 { 7.0 } else { 0.0 }, zone);
    let Some(f) = zone.parts(first) else {
        return f64::NAN;
    };
    let offset = |p: Parts| -js_round(-(p.offset as f64) / 15.0) * 15.0;
    let weeks = (thursday - first - (offset(f) - offset(t)) * 60000.0) / (7.0 * DAY);
    weeks.trunc() + 1.0
}
pub fn format(time: f64, template: &str, zone: &Zone) -> Option<String> {
    let Some(p) = zone.parts(time) else {
        return Some("Invalid Date".into());
    };
    let template = if template.is_empty() { "YYYY-MM-DDTHH:mm:ssZ" } else { template };
    if template == "YYYY-MM-DD" {
        return Some(iso_date(time, zone));
    }
    static ADVANCED: OnceLock<Regex> = OnceLock::new();
    static FORMAT: OnceLock<Regex> = OnceLock::new();
    let advanced = ADVANCED.get_or_init(|| Regex::new(r"\[([^\]]+)]|Q|wo|ww|w|WW|W|zzz|z|gggg|GGGG|Do|X|x|k{1,2}|S").unwrap());
    let regular =
        FORMAT.get_or_init(|| Regex::new(r"\[([^\]]+)]|YYYY|YY|M{1,4}|D{1,2}|d{1,4}|H{1,2}|h{1,2}|a|A|m{1,2}|s{1,2}|Z{1,2}|SSS").unwrap());
    let mut unsupported = false;
    let expanded = advanced.replace_all(template, |m: &regex::Captures| match &m[0] {
        "Q" => (p.month / 3 + 1).to_string(),
        "Do" => format!("[{}]", ordinal(p.day)),
        "GGGG" => zone
            .parts(add_days(time, 4.0 - if p.weekday == 0 { 7.0 } else { p.weekday as f64 }, zone))
            .map(|p| p.year.to_string())
            .unwrap_or("NaN".into()),
        "W" => js_num(iso_week(time, zone)),
        "WW" => {
            let w = iso_week(time, zone);
            if w.is_nan() { "NaN".into() } else { pad(w as i64, 2) }
        }
        "k" => if p.hours == 0 { 24 } else { p.hours }.to_string(),
        "kk" => pad(if p.hours == 0 { 24 } else { p.hours } as i64, 2),
        "X" => js_num((time / 1000.0).floor()),
        "x" => js_num(time),
        "gggg" | "wo" | "w" | "ww" | "z" | "zzz" => {
            unsupported = true;
            m[0].into()
        }
        _ => m[0].into(),
    });
    if unsupported {
        return None;
    }
    let offset = -js_round(-(p.offset as f64) / 15.0) as i32 * 15;
    let zone_string = format!(
        "{}{}:{}",
        if offset >= 0 { '+' } else { '-' },
        pad(offset.unsigned_abs() as i64 / 60, 2),
        pad(offset.unsigned_abs() as i64 % 60, 2)
    );
    Some(
        regular
            .replace_all(&expanded, |m: &regex::Captures| {
                if let Some(literal) = m.get(1) {
                    return literal.as_str().to_string();
                }
                match &m[0] {
                    "YY" => {
                        let y = p.year.to_string();
                        y[y.len().saturating_sub(2)..].into()
                    }
                    "YYYY" => pad(p.year as i64, 4),
                    "M" => (p.month + 1).to_string(),
                    "MM" => pad(p.month as i64 + 1, 2),
                    "MMM" => MONTHS[p.month as usize][..3].into(),
                    "MMMM" => MONTHS[p.month as usize].into(),
                    "D" => p.day.to_string(),
                    "DD" => pad(p.day as i64, 2),
                    "d" => p.weekday.to_string(),
                    "dd" => WEEKDAYS[p.weekday as usize][..2].into(),
                    "ddd" => WEEKDAYS[p.weekday as usize][..3].into(),
                    "dddd" => WEEKDAYS[p.weekday as usize].into(),
                    "H" => p.hours.to_string(),
                    "HH" => pad(p.hours as i64, 2),
                    "h" => if p.hours % 12 == 0 { 12 } else { p.hours % 12 }.to_string(),
                    "hh" => pad(if p.hours % 12 == 0 { 12 } else { p.hours % 12 } as i64, 2),
                    "a" => if p.hours < 12 { "am" } else { "pm" }.into(),
                    "A" => if p.hours < 12 { "AM" } else { "PM" }.into(),
                    "m" => p.minutes.to_string(),
                    "mm" => pad(p.minutes as i64, 2),
                    "s" => p.seconds.to_string(),
                    "ss" => pad(p.seconds as i64, 2),
                    "SSS" => pad(p.milliseconds as i64, 3),
                    "Z" => zone_string.clone(),
                    _ => zone_string.replace(':', ""),
                }
            })
            .into_owned(),
    )
}
#[derive(Default)]
struct Fields {
    year: Option<f64>,
    month: Option<f64>,
    day: Option<f64>,
    day_given: bool,
    hours: Option<f64>,
    minutes: Option<f64>,
    seconds: Option<f64>,
    ms: Option<f64>,
    week: Option<f64>,
    offset: Option<f64>,
    afternoon: Option<bool>,
}
pub fn parse_strict(input: &str, template: &str, now: f64, zone: &Zone) -> Option<f64> {
    let date = if template == "x" || template == "X" {
        let t = (js_number(input) * if template == "X" { 1000.0 } else { 1.0 }).trunc();
        valid(t).then_some(t)?
    } else {
        static TOKENS: OnceLock<Regex> = OnceLock::new();
        static LITERAL: OnceLock<Regex> = OnceLock::new();
        static DIGITS: OnceLock<Regex> = OnceLock::new();
        let digits = DIGITS.get_or_init(|| Regex::new(r"\d+").unwrap());
        let literals = LITERAL.get_or_init(|| Regex::new(r"\[[^\]]+]").unwrap());
        if literals.replace_all(template, "").contains(['L', 'l']) {
            return None;
        }
        let tokens = TOKENS.get_or_init(|| {
            Regex::new(r"(\[[^\[]*\])|([-_:/.,()\s]+)|(A|a|Q|YYYY|YY?|ww?|MM?M?M?|Do|DD?|hh?|HH?|mm?|ss?|S{1,3}|z|ZZ?)").unwrap()
        });
        let mut fields = Fields::default();
        let mut rest = input.to_string();
        let mut start = 0;
        let mut any = false;
        for token in tokens.find_iter(template) {
            any = true;
            let token = token.as_str();
            let pattern = match token {
                "Q" | "S" => 0,
                "SS" | "DD" | "MM" | "ww" | "YY" => 1,
                "SSS" => 2,
                "YYYY" => 3,
                "s" | "ss" | "m" | "mm" | "H" | "h" | "HH" | "hh" | "D" | "w" | "M" => 4,
                "Y" => 5,
                "Z" | "ZZ" => 6,
                "A" | "a" | "MMM" | "MMMM" | "Do" => 7,
                _ => {
                    start += token
                        .strip_prefix('[')
                        .and_then(|s| s.strip_suffix(']'))
                        .unwrap_or(token)
                        .encode_utf16()
                        .count();
                    continue;
                }
            };
            let mut used = 0;
            let from = rest
                .char_indices()
                .find_map(|(i, c)| {
                    if used >= start {
                        Some(i)
                    } else {
                        used += c.len_utf16();
                        None
                    }
                })
                .unwrap_or(rest.len());
            static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
            let re = &PATTERNS.get_or_init(|| {
                [
                    r"[0-9]",
                    r"[0-9][0-9]",
                    r"[0-9]{3}",
                    r"[0-9]{4}",
                    r"[0-9][0-9]?",
                    r"[+-]?[0-9]+",
                    r"[+-][0-9][0-9]:?([0-9][0-9])?|Z",
                    r"[0-9]*[^-_:/,()\s0-9]+",
                ]
                .iter()
                .map(|p| Regex::new(p).unwrap())
                .collect()
            })[pattern];
            let found = re.find(&rest[from..])?.as_str().to_string();
            let value = js_number(&found);
            match token {
                "A" => fields.afternoon = Some(found == "PM"),
                "a" => fields.afternoon = Some(found == "pm"),
                "Q" => fields.month = Some((value - 1.0) * 3.0 + 1.0),
                "S" => fields.ms = Some(value * 100.0),
                "SS" => fields.ms = Some(value * 10.0),
                "SSS" => fields.ms = Some(value),
                "s" | "ss" => fields.seconds = Some(value),
                "m" | "mm" => fields.minutes = Some(value),
                "H" | "h" | "HH" | "hh" => fields.hours = Some(value),
                "D" | "DD" => fields.day = Some(value),
                "w" | "ww" => fields.week = Some(value),
                "M" | "MM" => fields.month = Some(value),
                "Y" | "YYYY" => fields.year = Some(value),
                "YY" => fields.year = Some(value + if value > 68.0 { 1900.0 } else { 2000.0 }),
                "MMM" | "MMMM" => {
                    let i = MONTHS
                        .iter()
                        .position(|m| if token == "MMM" { m[..3] == found } else { *m == found })?;
                    fields.month = Some(i as f64 + 1.0);
                }
                "Do" => {
                    fields.day = Some(js_number(digits.find(&found)?.as_str()));
                    fields.day_given = true;
                    for i in 1..=31 {
                        if ordinal(i) == found {
                            fields.day = Some(i as f64);
                        }
                    }
                }
                "Z" | "ZZ" => {
                    if found == "Z" {
                        fields.offset = Some(0.0);
                    } else {
                        let digits = found[1..].replace(':', "");
                        let h = js_number(&digits[..2]);
                        let m = if digits.len() > 2 { js_number(&digits[2..]) } else { 0.0 };
                        fields.offset = Some((h * 60.0 + m) * if found.starts_with('+') { -1.0 } else { 1.0 });
                    }
                }
                _ => {}
            }
            rest = rest.replacen(&found, "", 1);
        }
        if !any {
            return None;
        }
        if let (Some(pm), Some(hours)) = (fields.afternoon, fields.hours) {
            fields.hours = Some(if pm && hours < 12.0 {
                hours + 12.0
            } else if !pm && hours == 12.0 {
                0.0
            } else {
                hours
            });
        }
        let p = zone.parts(now)?;
        let truth = |v: Option<f64>| v.is_some_and(|v| v != 0.0 && !v.is_nan());
        let day = if fields.day_given || truth(fields.day) {
            fields.day?
        } else if !truth(fields.year) && !truth(fields.month) {
            p.day as f64
        } else {
            1.0
        };
        let year = if truth(fields.year) { fields.year? } else { p.year as f64 };
        let month = if truth(fields.year) && !truth(fields.month) {
            0.0
        } else if fields.month.is_some_and(|m| m > 0.0) {
            fields.month? - 1.0
        } else {
            p.month as f64
        };
        let h = fields.hours.unwrap_or(0.0);
        let m = fields.minutes.unwrap_or(0.0);
        let s = fields.seconds.unwrap_or(0.0);
        let ms = fields.ms.unwrap_or(0.0);
        if let Some(offset) = fields.offset {
            make(year, month, day, h, m, s, ms + offset * 60000.0, &Zone::Named(chrono_tz::UTC), true)?
        } else {
            if truth(fields.week) {
                return None;
            }
            make(year, month, day, h, m, s, ms, zone, true)?
        }
    };
    (format(date, template, zone).as_deref() == Some(input)).then_some(date)
}
pub fn fallback(input: &str, zone: &Zone) -> Option<f64> {
    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(input) {
        return Some(date.timestamp_millis() as f64);
    }
    if let Ok(date) = chrono::DateTime::parse_from_rfc2822(input) {
        return Some(date.timestamp_millis() as f64);
    }
    static ISO: OnceLock<Regex> = OnceLock::new();
    let re = ISO.get_or_init(|| {
        Regex::new(r"^([+-]\d{6}|\d{4})(?:-(\d{2})(?:-(\d{2}))?)?(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?)?(Z)?$").unwrap()
    });
    if let Some(c) = re.captures(input) {
        if c.get(1)?.as_str() == "-000000" {
            return None;
        }
        let n = |i| c.get(i).map(|v| js_number(v.as_str()));
        let utc = c.get(4).is_none() || c.get(8).is_some();
        let ms = c
            .get(7)
            .map(|v| js_number(&format!("{:<03}", v.as_str()).replace(' ', "0")))
            .unwrap_or(0.0);
        let month = n(2).unwrap_or(1.0);
        let day = n(3).unwrap_or(1.0);
        let hours = n(4).unwrap_or(0.0);
        let minutes = n(5).unwrap_or(0.0);
        let seconds = n(6).unwrap_or(0.0);
        if !(1.0..=12.0).contains(&month)
            || !(1.0..=31.0).contains(&day)
            || hours > 24.0
            || minutes >= 60.0
            || seconds >= 60.0
            || (hours == 24.0 && (minutes != 0.0 || seconds != 0.0 || ms != 0.0))
        {
            return None;
        }
        return make(
            n(1)?,
            n(2).unwrap_or(1.0) - 1.0,
            n(3).unwrap_or(1.0),
            n(4).unwrap_or(0.0),
            n(5).unwrap_or(0.0),
            n(6).unwrap_or(0.0),
            ms,
            if utc { &Zone::Named(chrono_tz::UTC) } else { zone },
            false,
        );
    }
    for template in [
        "MMM D, YYYY",
        "MMMM D, YYYY",
        "D MMM YYYY",
        "M/D/YYYY",
        "YYYY/M/D",
        "M-D-YYYY",
        "MMM D YYYY",
    ] {
        if let Some(date) = parse_strict(input, template, 0.0, zone) {
            return Some(date);
        }
    }
    legacy_date(input, zone)
}

// The host Date fallback accepts loose calendar components as well as the
// strict formats above. Keep its token rules separate from dayjs formatting:
// unknown words may precede a date, a month is identified by its first three
// letters, and a hyphen belongs to the number immediately before it.
fn legacy_date(input: &str, zone: &Zone) -> Option<f64> {
    let chars: Vec<char> = input.chars().collect();
    let mut at = 0;
    let mut numbers = Vec::new();
    let mut month = None;
    let mut offset = None;
    let mut time = None;
    let mut meridiem = None;
    let number = |at: &mut usize| -> Option<f64> {
        let start = *at;
        while chars.get(*at).is_some_and(char::is_ascii_digit) {
            *at += 1;
        }
        if start == *at {
            None
        } else {
            chars[start..*at].iter().collect::<String>().parse().ok()
        }
    };
    while at < chars.len() {
        let c = chars[at];
        if crate::util::is_js_space(c) || matches!(c, '/' | '.' | ',') {
            at += 1;
            continue;
        }
        if c == '(' {
            let mut depth = 1;
            at += 1;
            while at < chars.len() && depth > 0 {
                if chars[at] == '(' {
                    depth += 1;
                } else if chars[at] == ')' {
                    depth -= 1;
                }
                at += 1;
            }
            continue;
        }
        if c.is_ascii_digit() {
            let value = number(&mut at)?;
            if chars.get(at) == Some(&':') {
                if time.is_some() {
                    return None;
                }
                at += 1;
                let minutes = number(&mut at)?;
                let mut seconds = 0.;
                let mut ms = 0.;
                if chars.get(at) == Some(&':') {
                    at += 1;
                    seconds = number(&mut at)?;
                    if chars.get(at) == Some(&'.') {
                        at += 1;
                        let begin = at;
                        number(&mut at)?;
                        let fraction: String = chars[begin..at].iter().take(3).collect();
                        ms = js_number(&format!("{fraction:0<3}"));
                    }
                }
                if value > 24. || minutes > 59. || seconds > 59. || (value == 24. && (minutes != 0. || seconds != 0. || ms != 0.)) {
                    return None;
                }
                time = Some((value, minutes, seconds, ms));
            } else {
                if time.is_some() || numbers.len() == 3 {
                    return None;
                }
                numbers.push(value);
                if chars.get(at) == Some(&'-') {
                    at += 1;
                }
            }
            continue;
        }
        if c >= 'A' {
            let start = at;
            while at < chars.len() && chars[at] >= 'A' && !crate::util::is_js_space(chars[at]) {
                at += 1;
            }
            let word = chars[start..at].iter().collect::<String>().to_ascii_lowercase();
            if let Some(m) = MONTHS.iter().position(|m| word.starts_with(&m[..3].to_ascii_lowercase())) {
                month = Some(m as f64 + 1.);
            } else if matches!(word.as_str(), "utc" | "gmt" | "z") {
                offset = Some(0.);
            } else if let Some(minutes) = match word.as_str() {
                "est" => Some(-300.),
                "edt" => Some(-240.),
                "cst" => Some(-360.),
                "cdt" => Some(-300.),
                "mst" => Some(-420.),
                "mdt" => Some(-360.),
                "pst" => Some(-480.),
                "pdt" => Some(-420.),
                _ => None,
            } {
                offset = Some(minutes);
            } else if matches!(word.as_str(), "am" | "pm") && time.is_some() {
                if meridiem.is_some() {
                    return None;
                }
                meridiem = Some(word == "pm");
            } else if !numbers.is_empty() || time.is_some() || chars.get(at).is_some_and(char::is_ascii_digit) {
                return None;
            }
            continue;
        }
        if matches!(c, '+' | '-') && (offset.is_some() || time.is_some()) {
            let sign = if c == '-' { -1. } else { 1. };
            at += 1;
            let begin = at;
            let value = number(&mut at)?;
            let digits = at - begin;
            let minutes = if chars.get(at) == Some(&':') {
                at += 1;
                value * 60. + number(&mut at)?
            } else if digits <= 2 {
                value * 60.
            } else {
                (value / 100.).floor() * 60. + value % 100.
            };
            offset = Some(sign * minutes);
            continue;
        }
        if !numbers.is_empty() || time.is_some() {
            return None;
        }
        at += 1;
    }
    if numbers.is_empty() {
        return None;
    }
    let first = numbers[0];
    let (mut year, month, day) = if let Some(month) = month {
        if numbers.len() > 2 {
            return None;
        }
        if (1.0..=31.).contains(&first) {
            (numbers.get(1).copied().unwrap_or(1.), month, first)
        } else {
            (first, month, numbers.get(1).copied().unwrap_or(1.))
        }
    } else if !(1.0..=31.).contains(&first) {
        (first, numbers.get(1).copied().unwrap_or(1.), numbers.get(2).copied().unwrap_or(1.))
    } else {
        (numbers.get(2).copied().unwrap_or(1.), first, numbers.get(1).copied().unwrap_or(1.))
    };
    if !(1.0..=12.).contains(&month) || !(1.0..=31.).contains(&day) {
        return None;
    }
    if (0.0..100.).contains(&year) {
        year += if year < 50. { 2000. } else { 1900. };
    }
    let (mut hours, minutes, seconds, ms) = time.unwrap_or((0., 0., 0., 0.));
    if let Some(pm) = meridiem {
        if hours > 12. {
            return None;
        }
        hours = hours % 12. + if pm { 12. } else { 0. };
    }
    let utc = Zone::Named(chrono_tz::UTC);
    let value = make(
        year,
        month - 1.,
        day,
        hours,
        minutes,
        seconds,
        ms,
        if offset.is_some() { &utc } else { zone },
        false,
    )? - offset.unwrap_or(0.) * 60000.;
    valid(value).then_some(value)
}

#[cfg(test)]
mod fallback_tests {
    use super::*;
    #[test]
    fn legacy_calendar_words_and_separators_match_host_dates() {
        let zone = Zone::Named(chrono_tz::UTC);
        for (input, expected) in [
            ("a-7", Some("2001-07-01")),
            ("d-7", Some("2001-07-01")),
            ("abc7", None),
            ("junk7", Some("2001-06-07")),
            ("jan-7", Some("2001-01-07")),
            ("foo_7", None),
            ("2017-07- 3", Some("2017-07-03")),
            ("2017-07\u{a0}20", Some("2017-07-20")),
            ("2017 -07-20", None),
            ("2017--07--03", None),
            ("7-3", Some("2001-07-03")),
            ("13-3", None),
            ("32-3", Some("2032-03-01")),
            ("50-3", Some("1950-03-01")),
            ("01/02/03", Some("2003-01-02")),
            ("jan feb 7", Some("2001-02-07")),
            ("7 jan 8", Some("2008-01-07")),
            ("jan0", Some("2000-01-01")),
            ("word-7 blah", None),
            ("-7", Some("2001-07-01")),
            ("7-", Some("2001-07-01")),
            ("7--", None),
        ] {
            assert_eq!(fallback(input, &zone).map(|d| iso_date(d, &zone)).as_deref(), expected, "{input}");
        }
    }
}
