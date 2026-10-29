use crate::util::num;
pub const OVERHANG: f64 = 7.;
pub fn mask(directions: &[String]) -> u8 {
    let mut mask = 0;
    for dir in directions {
        mask |= match dir.trim() {
            "right" => 1,
            "left" => 2,
            "up" => 4,
            "down" => 8,
            "x" => 3,
            "y" => 12,
            _ => 0,
        };
    }
    mask
}
pub fn axis(mask: u8) -> u8 {
    if mask != 0 && mask & 12 == 0 {
        1
    } else if mask != 0 && mask & 3 == 0 {
        2
    } else {
        0
    }
}
fn bit(mask: u8, flag: u8) -> f64 {
    if mask & flag != 0 { 1. } else { 0. }
}
pub fn size(mask: u8, tw: f64, th: f64) -> (f64, f64, f64, f64) {
    let across = bit(mask, 2) - bit(mask, 1);
    let along = bit(mask, 4) - bit(mask, 8);
    match axis(mask) {
        1 => {
            let h = (th + 8.).max(14.) + 2. * OVERHANG;
            let depth = (h / 2.).min(20.);
            (
                (tw + 12.).max(16.) + depth * if mask == 3 { 2. } else { 1. },
                h,
                across * depth / 2.,
                0.,
            )
        }
        2 => {
            let w = (tw + 16.).max(14.) + 2. * OVERHANG;
            let depth = (w / 2.).min(20.);
            (
                w,
                (th + 8.).max(16.) + depth * if mask == 12 { 2. } else { 1. },
                0.,
                along * depth / 2.,
            )
        }
        _ => (
            (tw + 16.).max(24.) + 12. * (bit(mask, 2) + bit(mask, 1)),
            (th + 8.).max(24.) + 12. * (bit(mask, 4) + bit(mask, 8)),
            across * 12. / 2.,
            along * 12. / 2.,
        ),
    }
}
pub fn draw(mask: u8, w: f64, h: f64, attrs: &str) -> String {
    let (x, y) = (w / 2., h / 2.);
    let mut p;
    match axis(mask) {
        1 => {
            let depth = y.min(20.);
            let s = y - OVERHANG;
            let x0 = if mask & 2 != 0 { -x + depth } else { -x };
            let x1 = if mask & 1 != 0 { x - depth } else { x };
            p = vec![x0, -s, x1, -s];
            if mask & 1 != 0 {
                p.extend_from_slice(&[x1, -y, x, 0., x1, y]);
            }
            p.extend_from_slice(&[x1, s, x0, s]);
            if mask & 2 != 0 {
                p.extend_from_slice(&[x0, y, -x, 0., x0, -y]);
            }
        }
        2 => {
            let depth = x.min(20.);
            let s = x - OVERHANG;
            let y0 = if mask & 4 != 0 { -y + depth } else { -y };
            let y1 = if mask & 8 != 0 { y - depth } else { y };
            p = vec![s, y0, s, y1];
            if mask & 8 != 0 {
                p.extend_from_slice(&[x, y1, 0., y, -x, y1]);
            }
            p.extend_from_slice(&[-s, y1, -s, y0]);
            if mask & 4 != 0 {
                p.extend_from_slice(&[-x, y0, 0., -y, x, y0]);
            }
        }
        _ => {
            let x0 = if mask & 2 != 0 { -x + 12. } else { -x };
            let x1 = if mask & 1 != 0 { x - 12. } else { x };
            let y0 = if mask & 4 != 0 { -y + 12. } else { -y };
            let y1 = if mask & 8 != 0 { y - 12. } else { y };
            let (cx, cy) = ((x0 + x1) / 2., (y0 + y1) / 2.);
            let (kx, ky) = (10f64.min((x1 - x0) / 2.), 10f64.min((y1 - y0) / 2.));
            p = vec![x0, y0];
            if mask & 4 != 0 {
                p.extend_from_slice(&[cx - kx, y0, cx, -y, cx + kx, y0]);
            }
            p.extend_from_slice(&[x1, y0]);
            if mask & 1 != 0 {
                p.extend_from_slice(&[x1, cy - ky, x, cy, x1, cy + ky]);
            }
            p.extend_from_slice(&[x1, y1]);
            if mask & 8 != 0 {
                p.extend_from_slice(&[cx + kx, y1, cx, y, cx - kx, y1]);
            }
            p.extend_from_slice(&[x0, y1]);
            if mask & 2 != 0 {
                p.extend_from_slice(&[x0, cy + ky, -x, cy, x0, cy - ky]);
            }
        }
    }
    let points = p
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| format!("{},{}", num(p[0]), num(p[1])))
        .collect::<Vec<_>>()
        .join(" ");
    format!("<polygon points=\"{points}\" stroke-linejoin=\"round\"{attrs}/>")
}
