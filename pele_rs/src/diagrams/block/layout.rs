use super::render::View;
#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub children: Vec<usize>,
    pub columns: f64,
    pub span: f64,
    pub space: bool,
    pub head: f64,
    pub nw: f64,
    pub nh: f64,
    pub col: usize,
    pub row: usize,
    pub cols: usize,
    pub heights: Vec<f64>,
    pub row_gaps: Vec<f64>,
    pub col_gaps: Vec<f64>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
pub fn layout(v: &mut [View], gap: f64) -> (f64, f64) {
    for i in (0..v.len()).rev() {
        if v[i].children.is_empty() {
            continue;
        }
        let columns = v[i].columns;
        let inset = if i == 0 { 0. } else { gap };
        let mut heights = Vec::new();
        let (mut position, mut cols, mut cw, mut tallest) = (0usize, 0usize, 0f64, 0f64);
        let children = v[i].children.clone();
        for child in children {
            let (px, py) = if columns < 0. {
                (position, 0)
            } else {
                (position % (columns as usize), position / (columns as usize))
            };
            let c = &mut v[child];
            let mut span = if c.span >= 1. { c.span.min(1000.) } else { 1. };
            if columns > 0. && span > columns - px as f64 {
                span = columns - px as f64;
            }
            c.span = span;
            c.col = px;
            c.row = py;
            position += span as usize;
            cols = cols.max(px + span as usize);
            if py == heights.len() {
                heights.push(0.);
            }
            if c.children.is_empty() {
                heights[py] = -1.;
            } else if heights[py] >= 0. && c.nh > heights[py] {
                heights[py] = c.nh;
            }
            if c.space {
                continue;
            }
            let width = (c.nw - gap * (span - 1.)) / span;
            cw = cw.max(width);
            tallest = tallest.max(c.nh);
        }
        let mut height = (heights.len() as f64 - 1.) * gap + v[i].row_gaps.iter().sum::<f64>();
        for h in &mut heights {
            if *h < 0. {
                *h = tallest;
            }
            height += *h;
        }
        let c = &mut v[i];
        c.cols = cols;
        c.heights = heights;
        c.nw =
            c.nw.max(cols as f64 * cw + (cols as f64 - 1.) * gap + c.col_gaps.iter().sum::<f64>() + 2. * inset);
        c.nh = height + 2. * inset + c.head;
    }
    v[0].w = v[0].nw;
    v[0].h = v[0].nh;
    v[0].x = v[0].w / 2.;
    v[0].y = v[0].h / 2.;
    for i in 0..v.len() {
        if v[i].children.is_empty() {
            continue;
        }
        let inset = if i == 0 { 0. } else { gap };
        let c = &v[i];
        let rows = c.heights.len();
        let cw = (c.w - 2. * inset - (c.cols as f64 - 1.) * gap - c.col_gaps.iter().sum::<f64>()) / c.cols as f64;
        let room = c.h - 2. * inset - c.head - (rows as f64 - 1.) * gap - c.row_gaps.iter().sum::<f64>();
        let mut offsets = Vec::new();
        if !c.col_gaps.is_empty() {
            let mut sum = 0.;
            for col in 0..c.cols {
                offsets.push(sum);
                sum += c.col_gaps.get(col).copied().unwrap_or(0.);
            }
        }
        let needed = c.heights.iter().sum::<f64>();
        let scale = if needed > 0. { room / needed } else { 0. };
        let share = if needed > 0. { 0. } else { room / rows as f64 };
        let left = c.x - c.w / 2. + inset;
        let mut top = c.y - c.h / 2. + inset + c.head;
        let mut row = 0;
        let children = c.children.clone();
        let heights = c.heights.clone();
        let row_gaps = c.row_gaps.clone();
        for child in children {
            let c = &mut v[child];
            while row < c.row {
                top += heights[row] * scale + share + gap + row_gaps.get(row).copied().unwrap_or(0.);
                row += 1;
            }
            let before = offsets.get(c.col).copied().unwrap_or(0.);
            c.w = cw * c.span + gap * (c.span - 1.) + offsets.get(c.col + c.span as usize - 1).copied().unwrap_or(0.) - before;
            c.h = heights[row] * scale + share;
            c.x = left + c.col as f64 * (cw + gap) + before + c.w / 2.;
            c.y = top + c.h / 2.;
        }
    }
    (v[0].w, v[0].h)
}
