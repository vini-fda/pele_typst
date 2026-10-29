use crate::{
    svg::edges::{marker, marker_trim},
    util::num,
};
pub fn trim(kind: &str) -> f64 {
    match kind {
        "extension" => 12.,
        "composition" | "aggregation" => 16.,
        "lollipop" => 10.,
        "dependency" => marker_trim("arrow_point"),
        _ => 0.,
    }
}
pub fn draw(kind: &str, x: f64, y: f64, dx: f64, dy: f64, color: &str) -> String {
    let (nx, ny) = (-dy, dx);
    match kind {
        "extension" => {
            let (bx, by) = (x - dx * 12., y - dy * 12.);
            format!(
                "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}Z\" stroke-linejoin=\"round\"/>",
                num(x),
                num(y),
                num(bx + nx * 6.),
                num(by + ny * 6.),
                num(bx - nx * 6.),
                num(by - ny * 6.)
            )
        }
        "composition" | "aggregation" => {
            let (mx, my) = (x - dx * 8., y - dy * 8.);
            format!(
                "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}L{},{}Z\" stroke-linejoin=\"round\"{}/>",
                num(x),
                num(y),
                num(mx + nx * 5.),
                num(my + ny * 5.),
                num(x - dx * 16.),
                num(y - dy * 16.),
                num(mx - nx * 5.),
                num(my - ny * 5.),
                if kind == "composition" {
                    format!(" fill=\"{color}\"")
                } else {
                    String::new()
                }
            )
        }
        "lollipop" => format!(
            "<circle class=\"pele-marker\" cx=\"{}\" cy=\"{}\" r=\"5\"/>",
            num(x - dx * 5.),
            num(y - dy * 5.)
        ),
        "dependency" => marker("arrow_point", x, y, dx, dy, color, "none"),
        _ => String::new(),
    }
}
