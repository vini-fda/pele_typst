//! d3-compatible local calendar intervals and English axis formatting.
use super::dates::{self, DAY, MONTHS, WEEKDAYS, Zone};
use crate::util::{js_num, js_number};
const SECOND: f64 = 1000.0;
const MINUTE: f64 = 60000.0;
const HOUR: f64 = 3600000.0;
const WEEK: f64 = 7.0 * DAY;
const MONTH: f64 = 30.0 * DAY;
const YEAR: f64 = 365.0 * DAY;
#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}
#[derive(Clone, Copy)]
struct Calendar {
    unit: Unit,
    step: f64,
    week_start: u32,
}
impl Calendar {
    fn base_floor(&self, time: f64, zone: &Zone) -> f64 {
        let Some(p) = zone.parts(time) else {
            return f64::NAN;
        };
        match self.unit {
            Unit::Millisecond => time,
            Unit::Second => time - p.milliseconds as f64,
            Unit::Minute => time - p.milliseconds as f64 - p.seconds as f64 * SECOND,
            Unit::Hour => time - p.milliseconds as f64 - p.seconds as f64 * SECOND - p.minutes as f64 * MINUTE,
            Unit::Day => dates::start_day(time, zone),
            Unit::Week => dates::start_day(dates::add_days(time, -(((p.weekday + 7 - self.week_start) % 7) as f64), zone), zone),
            Unit::Month => dates::start_day(dates::set(time, zone, None, None, Some(1.0), None, None, None, None), zone),
            Unit::Year => dates::start_day(dates::set(time, zone, None, Some(0.0), Some(1.0), None, None, None, None), zone),
        }
    }
    fn base_offset(&self, time: f64, step: f64, zone: &Zone) -> f64 {
        let Some(p) = zone.parts(time) else {
            return f64::NAN;
        };
        match self.unit {
            Unit::Millisecond => time + step,
            Unit::Second => time + step * SECOND,
            Unit::Minute => time + step * MINUTE,
            Unit::Hour => time + step * HOUR,
            Unit::Day => dates::add_days(time, step, zone),
            Unit::Week => dates::add_days(time, step * 7.0, zone),
            Unit::Month => dates::set(time, zone, None, Some(p.month as f64 + step), None, None, None, None, None),
            Unit::Year => dates::set(time, zone, Some(p.year as f64 + step), None, None, None, None, None, None),
        }
    }
    fn field(&self, time: f64, zone: &Zone) -> f64 {
        let Some(p) = zone.parts(time) else {
            return f64::NAN;
        };
        match self.unit {
            Unit::Second => p.seconds as f64,
            Unit::Minute => p.minutes as f64,
            Unit::Hour => p.hours as f64,
            Unit::Day => p.day as f64 - 1.0,
            Unit::Month => p.month as f64,
            _ => f64::NAN,
        }
    }
    fn floor(&self, time: f64, zone: &Zone) -> f64 {
        if self.step <= 1.0 {
            return self.base_floor(time, zone);
        }
        match self.unit {
            Unit::Millisecond => (time / self.step).floor() * self.step,
            Unit::Year => {
                let Some(p) = zone.parts(time) else {
                    return f64::NAN;
                };
                self.base_floor(
                    dates::set(
                        time,
                        zone,
                        Some((p.year as f64 / self.step).floor() * self.step),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ),
                    zone,
                )
            }
            Unit::Week => {
                let epoch = self.base_floor(0.0, zone);
                let d = self.base_floor(time, zone);
                self.base_offset(d, -span(epoch, d, WEEK, zone).rem_euclid(self.step), zone)
            }
            _ => {
                let mut d = self.base_floor(time, zone);
                while dates::valid(d) && self.field(d, zone) % self.step != 0.0 {
                    d = self.base_floor(d - 1.0, zone);
                }
                d
            }
        }
    }
    fn offset(&self, time: f64, n: f64, zone: &Zone) -> f64 {
        if self.step <= 1.0 {
            return self.base_offset(time, n, zone);
        }
        if matches!(self.unit, Unit::Millisecond | Unit::Year | Unit::Week) {
            return self.base_offset(time, n * self.step, zone);
        }
        let mut d = time;
        for _ in 0..n as usize {
            if !dates::valid(d) {
                break;
            }
            loop {
                d = self.base_offset(d, 1.0, zone);
                if !dates::valid(d) || self.field(d, zone) % self.step == 0.0 {
                    break;
                }
            }
        }
        d
    }
    fn cycle(&self) -> f64 {
        match self.unit {
            Unit::Second | Unit::Minute => 60.0,
            Unit::Hour => 24.0,
            Unit::Day => 31.0,
            Unit::Month => 12.0,
            _ => f64::INFINITY,
        }
    }
    fn ms(&self) -> f64 {
        match self.unit {
            Unit::Millisecond => 1.0,
            Unit::Second => SECOND,
            Unit::Minute => MINUTE,
            Unit::Hour => HOUR,
            Unit::Day => DAY,
            Unit::Week => WEEK,
            Unit::Month => MONTH,
            Unit::Year => YEAR,
        }
    }
}
fn span(start: f64, end: f64, unit: f64, zone: &Zone) -> f64 {
    let (Some(a), Some(b)) = (zone.parts(start), zone.parts(end)) else {
        return f64::NAN;
    };
    ((end - start + (b.offset - a.offset) as f64 * MINUTE) / unit).floor()
}
fn range(calendar: Calendar, start: f64, stop: f64, limit: f64, zone: &Zone) -> Option<Vec<f64>> {
    let mut d = calendar.floor(start - 1.0, zone);
    d = calendar.offset(d, 1.0, zone);
    d = calendar.floor(d, zone);
    let mut out = vec![];
    let mut previous = f64::NEG_INFINITY;
    while d <= stop && d > previous {
        if out.len() as f64 >= limit {
            return None;
        }
        previous = d;
        out.push(d);
        d = calendar.offset(d, 1.0, zone);
        d = calendar.floor(d, zone);
    }
    Some(out)
}
fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= std::f64::consts::SQRT_2 {
        2.0
    } else {
        1.0
    };
    10f64.powf(power) * factor
}
pub fn auto_ticks(start: f64, stop: f64, count: f64, zone: &Zone) -> Vec<f64> {
    use Unit::*;
    let auto = [
        (Second, 1.0, SECOND),
        (Second, 5.0, 5.0 * SECOND),
        (Second, 15.0, 15.0 * SECOND),
        (Second, 30.0, 30.0 * SECOND),
        (Minute, 1.0, MINUTE),
        (Minute, 5.0, 5.0 * MINUTE),
        (Minute, 15.0, 15.0 * MINUTE),
        (Minute, 30.0, 30.0 * MINUTE),
        (Hour, 1.0, HOUR),
        (Hour, 3.0, 3.0 * HOUR),
        (Hour, 6.0, 6.0 * HOUR),
        (Hour, 12.0, 12.0 * HOUR),
        (Day, 1.0, DAY),
        (Day, 2.0, 2.0 * DAY),
        (Week, 1.0, WEEK),
        (Month, 1.0, MONTH),
        (Month, 3.0, 3.0 * MONTH),
        (Year, 1.0, YEAR),
    ];
    let target = (stop - start) / count;
    let i = auto.iter().position(|v| v.2 > target).unwrap_or(auto.len());
    let (unit, step) = if i == auto.len() {
        (Year, tick_step(start / YEAR, stop / YEAR, count).max(1.0))
    } else if i == 0 {
        (Millisecond, tick_step(start, stop, count).max(1.0))
    } else {
        let j = if target / auto[i - 1].2 < auto[i].2 / target { i - 1 } else { i };
        (auto[j].0, auto[j].1)
    };
    range(Calendar { unit, step, week_start: 0 }, start, stop, 4.0 * count + 8.0, zone).unwrap_or_default()
}
pub fn interval_ticks(interval: &str, weekday: &str, start: f64, stop: f64, limit: f64, zone: &Zone) -> Option<Vec<f64>> {
    let end = interval.find(|c: char| !c.is_ascii_digit())?;
    if end == 0 || interval.starts_with('0') {
        return None;
    }
    let step = js_number(&interval[..end]);
    let unit = match &interval[end..] {
        "millisecond" => Unit::Millisecond,
        "second" => Unit::Second,
        "minute" => Unit::Minute,
        "hour" => Unit::Hour,
        "day" => Unit::Day,
        "week" => Unit::Week,
        "month" => Unit::Month,
        _ => return None,
    };
    let week_start = ["sunday", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday"]
        .iter()
        .position(|&s| s == weekday)
        .unwrap_or(0) as u32;
    let calendar = Calendar { unit, step, week_start };
    if (stop - start) / (step.min(calendar.cycle()) * calendar.ms()) > limit {
        return None;
    }
    range(calendar, start, stop, limit, zone)
}
fn pad(value: i64, fill: &str, width: usize) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let digits = value.unsigned_abs().to_string();
    format!("{sign}{}{digits}", fill.repeat(width.saturating_sub(digits.len())))
}
fn week_of_year(date: f64, week_start: u32, zone: &Zone) -> f64 {
    let week = Calendar {
        unit: Unit::Week,
        step: 1.0,
        week_start,
    };
    let year = Calendar {
        unit: Unit::Year,
        step: 1.0,
        week_start: 0,
    }
    .floor(date, zone);
    span(week.floor(year - 1.0, zone), week.floor(date, zone), WEEK, zone)
}
fn pad_number(value: f64, fill: &str, width: usize) -> String {
    if value.is_nan() {
        format!("{}NaN", fill.repeat(width.saturating_sub(3)))
    } else {
        pad(value as i64, fill, width)
    }
}
fn year_floor(date: f64, zone: &Zone) -> f64 {
    Calendar {
        unit: Unit::Year,
        step: 1.0,
        week_start: 0,
    }
    .floor(date, zone)
}
fn iso_thursday(date: f64, zone: &Zone) -> f64 {
    let Some(p) = zone.parts(date) else {
        return f64::NAN;
    };
    let week = Calendar {
        unit: Unit::Week,
        step: 1.0,
        week_start: 4,
    };
    let same = p.weekday >= 4 || p.weekday == 0;
    let mut d = week.floor(if same { date } else { date - 1.0 }, zone);
    if !same {
        d = week.floor(week.offset(d, 1.0, zone), zone);
    }
    d
}
fn iso_week(date: f64, zone: &Zone) -> f64 {
    let thursday = iso_thursday(date, zone);
    let year = year_floor(thursday, zone);
    let first = Calendar {
        unit: Unit::Week,
        step: 1.0,
        week_start: 4,
    }
    .floor(year, zone);
    span(first, thursday, WEEK, zone)
        + if zone.parts(year).is_some_and(|p| p.weekday == 4) {
            1.0
        } else {
            0.0
        }
}
pub fn time_format(specifier: &str, date: f64, zone: &Zone) -> String {
    let Some(p) = zone.parts(date) else {
        return "Invalid Date".into();
    };
    let mut out = String::new();
    let mut chars = specifier.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(mut c) = chars.next() else {
            break;
        };
        let fill = match c {
            '-' => "",
            '_' => " ",
            '0' => "0",
            _ => {
                if c == 'e' {
                    " "
                } else {
                    "0"
                }
            }
        };
        if matches!(c, '-' | '_' | '0') {
            let Some(next) = chars.next() else {
                break;
            };
            c = next;
        }
        let value = match c {
            'a' => WEEKDAYS[p.weekday as usize][..3].into(),
            'A' => WEEKDAYS[p.weekday as usize].into(),
            'b' => MONTHS[p.month as usize][..3].into(),
            'B' => MONTHS[p.month as usize].into(),
            'c' => time_format("%x, %X", date, zone),
            'd' | 'e' => pad(p.day as i64, fill, 2),
            'f' => pad(p.milliseconds as i64, fill, 3) + "000",
            'g' => pad_number(
                zone.parts(iso_thursday(date, zone)).map_or(f64::NAN, |p| (p.year % 100) as f64),
                fill,
                2,
            ),
            'G' => pad_number(
                zone.parts(iso_thursday(date, zone)).map_or(f64::NAN, |p| (p.year % 10000) as f64),
                fill,
                4,
            ),
            'H' => pad(p.hours as i64, fill, 2),
            'I' => pad(if p.hours % 12 == 0 { 12 } else { p.hours % 12 } as i64, fill, 2),
            'j' => pad_number(1.0 + span(year_floor(date, zone), dates::start_day(date, zone), DAY, zone), fill, 3),
            'L' => pad(p.milliseconds as i64, fill, 3),
            'm' => pad(p.month as i64 + 1, fill, 2),
            'M' => pad(p.minutes as i64, fill, 2),
            'p' => if p.hours >= 12 { "PM" } else { "AM" }.into(),
            'q' => (p.month / 3 + 1).to_string(),
            'Q' => js_num(date),
            's' => js_num((date / 1000.0).floor()),
            'S' => pad(p.seconds as i64, fill, 2),
            'u' => if p.weekday == 0 { 7 } else { p.weekday }.to_string(),
            'U' => pad_number(week_of_year(date, 0, zone), fill, 2),
            'V' => pad_number(iso_week(date, zone), fill, 2),
            'w' => p.weekday.to_string(),
            'W' => pad_number(week_of_year(date, 1, zone), fill, 2),
            'x' => time_format("%-m/%-d/%Y", date, zone),
            'X' => time_format("%-I:%M:%S %p", date, zone),
            'y' => pad((p.year % 100) as i64, fill, 2),
            'Y' => pad((p.year % 10000) as i64, fill, 4),
            'Z' => format!(
                "{}{}{}",
                if p.offset < 0 { '-' } else { '+' },
                pad(p.offset.unsigned_abs() as i64 / 60, "0", 2),
                pad(p.offset.unsigned_abs() as i64 % 60, "0", 2)
            ),
            _ => c.to_string(),
        };
        out.push_str(&value);
    }
    out
}
