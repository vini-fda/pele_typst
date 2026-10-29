use crate::TextMeasurer;
use crate::svg::builder::label_svg;
use crate::text::context::WithoutMath;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::BOLD;
use crate::util::num;

pub fn title_label(title: Option<&str>, measurer: &dyn TextMeasurer, size: f64, max_width: f64) -> Label {
    layout_label(title, false, &WithoutMath(measurer), size, max_width, BOLD)
}

pub struct Titled {
    pub width: f64,
    pub height: f64,
    pub content: String,
}

pub fn with_title(
    title: Option<&str>,
    measurer: &dyn TextMeasurer,
    size: f64,
    pad: f64,
    width: f64,
    height: f64,
    content: String,
) -> Titled {
    let label = title_label(title, measurer, size, 4000.0);
    if label.height == 0.0 {
        return Titled { width, height, content };
    }
    let shift = label.height + 12.0;
    let full = width.max((label.width + 2.0 * pad).ceil());
    Titled {
        width: full,
        height: (height + shift).ceil(),
        content: label_svg(
            &label,
            full / 2.0,
            pad + label.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\"",
        ) + &format!(
            "<g transform=\"translate({},{})\">{}</g>",
            num((full - width) / 2.0),
            num(shift),
            content
        ),
    }
}
