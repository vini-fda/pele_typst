use crate::util::num;

fn random(seed: f64) -> f64 {
    let mut t = (seed + 0x6d2b79f5_u32 as f64).trunc().rem_euclid(4294967296.0) as u32;
    t = (t ^ (t >> 15)).wrapping_mul(t | 1);
    t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
    (t ^ (t >> 14)) as f64 / 4294967296.0
}
pub fn seed(configured: f64, id: &str) -> f64 {
    if configured.is_finite() && configured != 0.0 {
        return configured;
    }
    let mut hash = 0_i32;
    for c in id.encode_utf16() {
        hash = hash.wrapping_shl(5).wrapping_sub(hash).wrapping_add(c as i32);
    }
    hash as f64
}
pub fn wave(from: f64, to: f64, center: f64, seed: f64, amplitude: f64, vertical: bool, pin_start: bool, pin_end: bool) -> Vec<f64> {
    let segments = 4;
    let step = (to - from) / segments as f64;
    let (point_step, control_step, control_shift) = if vertical { (17.0, 31.0, 7.0) } else { (23.0, 37.0, 11.0) };
    let mut along = vec![];
    let mut across = vec![];
    for i in 0..=segments {
        let pinned = (i == 0 && pin_start) || (i == segments && pin_end);
        along.push(from + i as f64 * step);
        across.push(if pinned {
            center
        } else {
            center + random(seed + i as f64 * point_step) * amplitude * 2.0 - amplitude
        });
    }
    let mut out = if vertical {
        vec![across[0], along[0]]
    } else {
        vec![along[0], across[0]]
    };
    for i in 0..segments {
        let mid = (along[i] + along[i + 1]) / 2.0;
        let offset = amplitude * 1.5 * if i % 2 == 0 { 1.0 } else { -1.0 } * random(seed + i as f64 * control_step + control_shift);
        if vertical {
            out.extend([across[i] + offset, mid, across[i + 1] - offset, mid, across[i + 1], along[i + 1]]);
        } else {
            out.extend([mid, across[i] + offset, mid, across[i + 1] - offset, along[i + 1], across[i + 1]]);
        }
    }
    out
}
pub fn cliff(cx: f64, top: f64, bottom: f64, amplitude: f64) -> Vec<f64> {
    let h = bottom - top;
    vec![
        cx,
        top,
        cx + amplitude,
        top + h * 0.2,
        cx - amplitude * 1.5,
        top + h * 0.55,
        cx + amplitude * 0.5,
        top + h * 0.75,
        cx - amplitude,
        top + h * 0.85,
        cx + amplitude * 0.3,
        top + h * 0.95,
        cx,
        bottom,
    ]
}
pub fn forward(curve: &[f64]) -> String {
    let mut d = String::new();
    for i in (2..curve.len()).step_by(6) {
        d += &format!(
            "C{},{} {},{} {},{}",
            num(curve[i]),
            num(curve[i + 1]),
            num(curve[i + 2]),
            num(curve[i + 3]),
            num(curve[i + 4]),
            num(curve[i + 5])
        );
    }
    d
}
pub fn backward(curve: &[f64]) -> String {
    let mut d = String::new();
    for i in (2..curve.len()).step_by(6).rev() {
        d += &format!(
            "C{},{} {},{} {},{}",
            num(curve[i + 2]),
            num(curve[i + 3]),
            num(curve[i]),
            num(curve[i + 1]),
            num(curve[i - 2]),
            num(curve[i - 1])
        );
    }
    d
}
pub fn path(curve: &[f64]) -> String {
    format!("M{},{}{}", num(curve[0]), num(curve[1]), forward(curve))
}
pub fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> String {
    let r = format!("{},{}", num(rx), num(ry));
    format!(
        "M{},{}A{r} 0 1,1 {},{}A{r} 0 1,1 {},{}Z",
        num(cx - rx),
        num(cy),
        num(cx + rx),
        num(cy),
        num(cx - rx),
        num(cy)
    )
}
