// Fitting a drawing to the width the host has, after pele's diagrams/common/fit-width.ts.

use crate::RenderOptions;
use crate::url::LinkInfo;

pub struct Rendered {
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub links: Vec<LinkInfo>,
}

// The width a title may take before it wraps.
pub fn title_room(options: &RenderOptions) -> f64 {
    match options.max_width {
        Some(a) if a > 0.0 => (a - 2.0 * options.padding.unwrap_or(8.0)).max(80.0),
        _ => 4000.0,
    }
}

const BREAKPOINT: f64 = 640.0;

// Draws across, and down instead when the host allows it, the width it has is under the
// breakpoint, across does not fit, and down is narrower.
pub fn turn_to_fit(options: &RenderOptions, draw: impl Fn(bool) -> Rendered) -> Rendered {
    let across = draw(false);
    let Some(available) = options.max_width.filter(|&a| a > 0.0) else {
        return across;
    };
    if options.auto_direction == Some(false) || across.width <= available {
        return across;
    }
    let breakpoint = options.direction_breakpoint.filter(|&b| b > 0.0).unwrap_or(BREAKPOINT);
    if available >= breakpoint {
        return across;
    }
    let down = draw(true);
    if down.width < across.width { down } else { across }
}

pub fn runs_across(direction: Option<&str>) -> bool {
    matches!(direction, Some("LR") | Some("RL"))
}

// Draws with room to spare, and tightly instead when that does not fit and tight is narrower.
pub fn tighten(options: &RenderOptions, draw: impl Fn(bool) -> Rendered) -> Rendered {
    let loose = draw(false);
    let Some(available) = options.max_width.filter(|&a| a > 0.0) else {
        return loose;
    };
    if loose.width <= available {
        return loose;
    }
    let tight = draw(true);
    if tight.width < loose.width { tight } else { loose }
}

// Draws a chart whose plot can be any width so that the whole diagram fits the width the host
// has. `narrow` tells the chart that it did not fit at its own width.
pub fn fit_width(options: &RenderOptions, natural: f64, least: f64, draw: impl Fn(f64, bool) -> Rendered) -> Rendered {
    let mut result = draw(natural, false);
    let Some(available) = options.max_width.filter(|&a| a > 0.0) else {
        return result;
    };
    if result.width <= available {
        return result;
    }
    let floor = least.min(natural);
    let mut plot = natural;
    result = draw(plot, true);
    let mut round = 0;
    while round < 3 && result.width > available && plot > floor {
        plot = floor.max(plot - (result.width - available));
        result = draw(plot, true);
        round += 1;
    }
    result
}
