use super::Subset;
use indexmap::IndexMap;
use std::f64::consts::{PI, TAU};
#[derive(Clone, Debug)]
pub struct Circle {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub r: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub x: f64,
    pub y: f64,
    pub margin: f64,
}
fn ensure_pairwise(subsets: &[Subset]) -> Vec<Subset> {
    let mut present = std::collections::HashSet::new();
    let mut sizes = std::collections::HashMap::new();
    for s in subsets {
        let mut ids = s.sets.clone();
        ids.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        present.insert(ids);
        if s.sets.len() == 1 {
            sizes.insert(s.sets[0].clone(), s.size);
        }
    }
    let mut out = subsets.to_vec();
    for s in subsets {
        if s.sets.len() < 3 {
            continue;
        }
        let mut ids = s.sets.clone();
        ids.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        for i in 0..ids.len() - 1 {
            for j in i + 1..ids.len() {
                let pair = vec![ids[i].clone(), ids[j].clone()];
                if !present.insert(pair.clone()) {
                    continue;
                }
                let size = match (sizes.get(&pair[0]), sizes.get(&pair[1])) {
                    (Some(a), Some(b)) => a.min(*b) / 4.0,
                    _ => 2.5,
                };
                out.push(Subset {
                    sets: pair,
                    size,
                    label: Some(String::new()),
                })
            }
        }
    }
    out
}
fn lens_area(r1: f64, r2: f64, d: f64) -> f64 {
    if d >= r1 + r2 {
        return 0.0;
    }
    let small = r1.min(r2);
    if d <= (r1 - r2).abs() {
        return PI * small * small;
    }
    let a1 = r1 * r1 * ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1)).acos();
    let a2 = r2 * r2 * ((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2)).acos();
    let a3 = 0.5 * ((-d + r1 + r2) * (d + r1 - r2) * (d - r1 + r2) * (d + r1 + r2)).max(0.0).sqrt();
    a1 + a2 - a3
}
fn distance_for_overlap(r1: f64, r2: f64, area: f64) -> f64 {
    let mut low = (r1 - r2).abs();
    let mut high = r1 + r2;
    if !(area > 0.0) {
        return high;
    }
    if area >= PI * r1.min(r2).powi(2) {
        return low;
    }
    for _ in 0..48 {
        let middle = (low + high) / 2.0;
        if lens_area(r1, r2, middle) > area {
            low = middle
        } else {
            high = middle
        }
    }
    (low + high) / 2.0
}
fn margin(circles: &[Circle], inside: &[usize], outside: &[usize], x: f64, y: f64, floor: f64) -> f64 {
    let mut best = f64::INFINITY;
    for (&i, inside) in inside.iter().map(|i| (i, true)).chain(outside.iter().map(|i| (i, false))) {
        let c = &circles[i];
        let dx = x - c.x;
        let dy = y - c.y;
        let distance = (dx * dx + dy * dy).sqrt();
        let room = if inside { c.r - distance } else { distance - c.r };
        if room < best {
            if room < floor {
                return room;
            }
            best = room
        }
    }
    best
}
fn consider(
    circles: &[Circle],
    inside: &[usize],
    outside: &[usize],
    p: &mut Spot,
    off: &mut f64,
    cx: f64,
    cy: f64,
    r: f64,
    x: f64,
    y: f64,
) -> bool {
    let m = margin(circles, inside, outside, x, y, p.margin - r * 1e-9);
    if !(m > p.margin - r * 1e-9) {
        return false;
    }
    let distance = (x - cx).hypot(y - cy);
    if m > p.margin + r * 1e-9 || distance < *off - r * 1e-9 {
        p.x = x;
        p.y = y;
        p.margin = m;
        *off = distance;
        true
    } else {
        false
    }
}
pub fn find_spot(circles: &[Circle], inside: &[usize], outside: &[usize]) -> Spot {
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut smallest = &circles[inside[0]];
    for &i in inside {
        let c = &circles[i];
        cx += c.x / inside.len() as f64;
        cy += c.y / inside.len() as f64;
        if c.r < smallest.r {
            smallest = c
        }
    }
    let r = smallest.r;
    let mut p = Spot {
        x: cx,
        y: cy,
        margin: margin(circles, inside, outside, cx, cy, f64::NEG_INFINITY),
    };
    let mut off = 0.0;
    for gy in -10..=10 {
        for gx in -10..=10 {
            if gx * gx + gy * gy <= 100 {
                consider(
                    circles,
                    inside,
                    outside,
                    &mut p,
                    &mut off,
                    cx,
                    cy,
                    r,
                    smallest.x + gx as f64 * r / 10.0,
                    smallest.y + gy as f64 * r / 10.0,
                );
            }
        }
    }
    let mut step = r / 10.0;
    for _ in 0..40 {
        if step <= r * 1e-6 {
            break;
        }
        let mut moved = false;
        for k in 0..8 {
            let angle = k as f64 * TAU / 8.0;
            let x = p.x + angle.cos() * step;
            let y = p.y + angle.sin() * step;
            if consider(circles, inside, outside, &mut p, &mut off, cx, cy, r, x, y) {
                moved = true
            }
        }
        if !moved {
            step /= 2.0
        }
    }
    p
}
pub fn layout(subsets: &[Subset]) -> Vec<Circle> {
    let mut sizes: IndexMap<String, f64> = IndexMap::new();
    for s in subsets {
        for id in &s.sets {
            sizes.entry(id.clone()).or_insert(10.0);
        }
    }
    for s in subsets {
        if s.sets.len() == 1 {
            sizes.insert(s.sets[0].clone(), s.size);
        }
    }
    let largest = sizes.values().filter(|s| s.is_finite()).copied().fold(0.0_f64, f64::max);
    let widest = (if largest == 0.0 { 10.0 } else { largest } / PI).sqrt();
    let mut circles = Vec::new();
    let mut index = std::collections::HashMap::new();
    for (id, size) in sizes {
        index.insert(id.clone(), circles.len());
        let r = if size.is_finite() && size > 0.0 { (size / PI).sqrt() } else { 0.0 };
        circles.push(Circle {
            id,
            x: 0.0,
            y: 0.0,
            r: r.max(widest * 0.3),
        })
    }
    let n = circles.len();
    if n < 2 {
        return circles;
    }
    let mut overlaps = IndexMap::new();
    for s in ensure_pairwise(subsets) {
        if s.sets.len() != 2 {
            continue;
        }
        let a = index[&s.sets[0]];
        let b = index[&s.sets[1]];
        if a != b {
            overlaps.insert(a.min(b) * n + a.max(b), s.size);
        }
    }
    let wanted: std::collections::HashMap<usize, f64> = overlaps
        .iter()
        .map(|(&key, &area)| (key, distance_for_overlap(circles[key / n].r, circles[key % n].r, area)))
        .collect();
    let gap = widest * 0.15;
    let distance = |a: usize, b: usize| wanted.get(&(a * n + b)).copied().unwrap_or(circles[a].r + circles[b].r + gap);
    if n == 2 {
        circles[1].x = distance(0, 1);
        return circles;
    }
    if n == 3 {
        let mut ab = distance(0, 1);
        let mut ac = distance(0, 2);
        let mut bc = distance(1, 2);
        let triple = subsets
            .iter()
            .any(|s| s.sets.iter().collect::<std::collections::HashSet<_>>().len() == 3);
        for _ in 0..40 {
            let x = if ab > 0.0 {
                (ab * ab + ac * ac - bc * bc) / (2.0 * ab)
            } else {
                0.0
            };
            circles[1].x = ab;
            circles[2].x = x.clamp(-ac, ac);
            circles[2].y = (ac * ac - x * x).max(0.0).sqrt();
            if !triple || find_spot(&circles, &[0, 1, 2], &[]).margin > widest * 0.12 {
                break;
            }
            ab *= 0.94;
            ac *= 0.94;
            bc *= 0.94
        }
        return circles;
    }
    let total: f64 = circles.iter().map(|c| 2.0 * c.r + gap).sum();
    let rounds = 300.min(3_000_000 / (n * n));
    if n <= 64 {
        let ring = total / TAU;
        for (i, c) in circles.iter_mut().enumerate() {
            let angle = i as f64 * TAU / n as f64 - PI / 2.0;
            c.x = angle.cos() * ring;
            c.y = angle.sin() * ring
        }
    } else {
        let cols = (n as f64).sqrt().ceil() as usize;
        let cell = 2.0 * widest + gap;
        for (i, c) in circles.iter_mut().enumerate() {
            c.x = (i % cols) as f64 * cell;
            c.y = (i / cols) as f64 * cell
        }
    }
    let mut dx = vec![0.0; n];
    let mut dy = vec![0.0; n];
    let mut pulls = vec![0; n];
    for _ in 0..rounds {
        dx.fill(0.0);
        dy.fill(0.0);
        pulls.fill(0);
        for a in 0..n {
            for b in a + 1..n {
                let overlap = wanted.get(&(a * n + b)).copied();
                let apart = circles[a].r + circles[b].r + gap;
                let mut ux = circles[b].x - circles[a].x;
                let mut uy = circles[b].y - circles[a].y;
                let mut length = ux.hypot(uy);
                if overlap.is_none() && length >= apart {
                    continue;
                }
                if length < 1e-9 {
                    ux = ((a + b) as f64).cos();
                    uy = ((a + b) as f64).sin();
                    length = 1.0
                }
                let shift = (length - overlap.unwrap_or(apart)) / 2.0 / length;
                dx[a] += ux * shift;
                dy[a] += uy * shift;
                dx[b] -= ux * shift;
                dy[b] -= uy * shift;
                pulls[a] += 1;
                pulls[b] += 1
            }
        }
        for i in 0..n {
            if pulls[i] > 0 {
                circles[i].x += dx[i] / pulls[i] as f64;
                circles[i].y += dy[i] / pulls[i] as f64
            }
        }
    }
    circles
}
pub fn region_path(circles: &[Circle]) -> String {
    use crate::util::num;
    let whole = |c: &Circle| {
        format!(
            "M{},{}A{},{} 0 1 1 {},{}A{},{} 0 1 1 {},{}Z",
            num(c.x - c.r),
            num(c.y),
            num(c.r),
            num(c.r),
            num(c.x + c.r),
            num(c.y),
            num(c.r),
            num(c.r),
            num(c.x - c.r),
            num(c.y)
        )
    };
    if circles.len() == 1 {
        return whole(&circles[0]);
    }
    let within = |x: f64, y: f64, a: usize, b: usize| {
        circles
            .iter()
            .enumerate()
            .all(|(i, c)| i == a || i == b || (x - c.x).hypot(y - c.y) <= c.r * (1.0 + 1e-9))
    };
    struct Corner {
        x: f64,
        y: f64,
        a: usize,
        b: usize,
    }
    let mut corners = Vec::new();
    for i in 0..circles.len() {
        for j in i + 1..circles.len() {
            let a = &circles[i];
            let b = &circles[j];
            let d = (b.x - a.x).hypot(b.y - a.y);
            if d >= a.r + b.r || d <= (a.r - b.r).abs() || d == 0.0 {
                continue;
            }
            let along = (a.r * a.r - b.r * b.r + d * d) / (2.0 * d);
            let up = (a.r * a.r - along * along).max(0.0).sqrt();
            let mx = a.x + (b.x - a.x) * along / d;
            let my = a.y + (b.y - a.y) * along / d;
            for sign in [1.0, -1.0] {
                let x = mx + sign * up * (b.y - a.y) / d;
                let y = my - sign * up * (b.x - a.x) / d;
                if within(x, y, i, j) {
                    corners.push(Corner { x, y, a: i, b: j })
                }
            }
        }
    }
    if corners.len() < 2 {
        let mut small = 0;
        for (i, c) in circles.iter().enumerate() {
            if c.r < circles[small].r {
                small = i
            }
        }
        let s = &circles[small];
        for (i, c) in circles.iter().enumerate() {
            if i != small && (c.x - s.x).hypot(c.y - s.y) + s.r > c.r * (1.0 + 1e-9) {
                return String::new();
            }
        }
        return whole(s);
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for c in &corners {
        cx += c.x / corners.len() as f64;
        cy += c.y / corners.len() as f64
    }
    corners.sort_by(|a, b| (a.y - cy).atan2(a.x - cx).total_cmp(&(b.y - cy).atan2(b.x - cx)));
    let mut d = String::new();
    for i in 0..corners.len() {
        let from = &corners[i];
        let to = &corners[(i + 1) % corners.len()];
        if i == 0 {
            d = format!("M{},{}", num(from.x), num(from.y))
        }
        let mut arc = None;
        let mut large = 0;
        let mut narrowest = f64::INFINITY;
        for ci in [from.a, from.b] {
            if ci != to.a && ci != to.b {
                continue;
            }
            let c = &circles[ci];
            let start = (from.y - c.y).atan2(from.x - c.x);
            let mut sweep = (to.y - c.y).atan2(to.x - c.x) - start;
            if sweep <= 0.0 {
                sweep += TAU
            }
            let mx = c.x + c.r * (start + sweep / 2.0).cos();
            let my = c.y + c.r * (start + sweep / 2.0).sin();
            let width = (mx - (from.x + to.x) / 2.0).hypot(my - (from.y + to.y) / 2.0);
            if width < narrowest {
                narrowest = width;
                arc = Some(c);
                large = if sweep > PI { 1 } else { 0 }
            }
        }
        d += &if let Some(c) = arc {
            format!("A{},{} 0 {large} 1 {},{}", num(c.r), num(c.r), num(to.x), num(to.y))
        } else {
            format!("L{},{}", num(to.x), num(to.y))
        }
    }
    d + "Z"
}
