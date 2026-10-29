use crate::util::num;
pub const MARKER_LENGTH: f64 = 18.0;
pub fn marker(kind: &str, x: f64, y: f64, dx: f64, dy: f64) -> String {
    let at = |back: f64, side: f64| format!("{},{}", num(x - dx * back - dy * side), num(y - dy * back + dx * side));
    let bar = |back| format!("M{}L{}", at(back, -5.0), at(back, 5.0));
    let foot = format!("M{}L{}L{}", at(0.0, -6.0), at(10.0, 0.0), at(0.0, 6.0));
    let ring = |back| {
        format!(
            "<circle class=\"pele-marker\" cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"var(--_bg)\"/>",
            num(x - dx * back),
            num(y - dy * back)
        )
    };
    let path = |d| format!("<path class=\"pele-marker\" d=\"{}\"/>", d);
    match kind {
        "only_one" => path(bar(6.0) + &bar(10.0)),
        "zero_or_one" => path(bar(6.0)) + &ring(13.5),
        "one_or_more" => path(foot + &bar(13.5)),
        "zero_or_more" => path(foot) + &ring(14.0),
        "md_parent" => format!(
            "<path class=\"pele-marker\" d=\"M{}L{}L{}L{}Z\" fill=\"var(--_bg)\"/>",
            at(0.0, 0.0),
            at(6.0, 4.0),
            at(12.0, 0.0),
            at(6.0, -4.0)
        ),
        _ => String::new(),
    }
}
