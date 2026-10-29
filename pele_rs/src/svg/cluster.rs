use std::collections::HashMap;

use super::builder::label_svg;
use crate::text::label::Label;
use crate::util::{js_round, num};

// The x of every route point, by its rounded y: the edges that cross a cluster's top border.
pub type Crossings = HashMap<i64, Vec<f64>>;

fn key(y: f64) -> i64 {
    js_round(y) as i64
}

pub fn mark_crossings(crossings: &mut Crossings, route: &[f64]) {
    let mut k = 0;
    while k + 1 < route.len() {
        crossings.entry(key(route[k + 1])).or_default().push(route[k]);
        k += 3;
    }
}

pub struct Box4 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

// Picks where a cluster title sits along the top edge so that edges entering there do not cross it.
pub fn cluster_title_x(c: &Box4, width: f64, crossings: &Crossings, pad: f64) -> f64 {
    if width == 0.0 {
        return c.x;
    }
    let top = c.y - c.h / 2.0;
    let left = c.x - c.w / 2.0;
    let half = width / 2.0;
    let candidates = [left + pad - 6.0 + half, c.x, left + c.w - pad + 6.0 - half];
    let Some(xs) = crossings.get(&key(top)) else { return candidates[0] };
    let mut best = candidates[0];
    let mut best_gap = -1.0;
    for x in candidates {
        let mut gap = f64::INFINITY;
        for &cx in xs {
            if cx > left && cx < left + c.w {
                gap = gap.min((cx - x).abs() - half);
            }
        }
        if gap >= 6.0 {
            return x;
        }
        if gap > best_gap + 0.5 {
            best_gap = gap;
            best = x;
        }
    }
    best
}

// Whether an edge that enters at the top runs through a title placed at `x`.
pub fn title_crossed(c: &Box4, width: f64, x: f64, crossings: &Crossings) -> bool {
    if width <= 0.0 {
        return false;
    }
    let Some(xs) = crossings.get(&key(c.y - c.h / 2.0)) else { return false };
    let left = c.x - c.w / 2.0;
    xs.iter().any(|&cx| cx > left && cx < left + c.w && (cx - x).abs() < width / 2.0 + 4.0)
}

// A title that an edge runs through, drawn over the edges on an outline in the color of the
// cluster behind it. `depth` counts the clusters it is in, each of which tints the background.
pub fn struck_title(label: &Label, x: f64, y: f64, attrs: &str, id: &str, depth: i32) -> String {
    let fill = format!("color-mix(in srgb,var(--_a) {}%,var(--_bg))", num(100.0 * (1.0 - 0.5f64.powi(depth))));
    format!(
        "<g class=\"pele-cluster-title\" data-id=\"{}\">{}{}</g>",
        id,
        label_svg(label, x, y, &format!(" fill=\"none\" stroke=\"{}\" stroke-width=\"5\" stroke-linejoin=\"round\"", fill)),
        label_svg(label, x, y, attrs)
    )
}
