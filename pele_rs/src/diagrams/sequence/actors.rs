use crate::svg::builder::label_svg;
use crate::svg::theme::ROUNDED;
use crate::text::label::Label;
use crate::util::num;
const SHAPE: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE: &str = " fill=\"none\" stroke=\"var(--_b)\"";
#[derive(Debug, Clone, Copy, Default)]
pub struct ActorSize {
    pub w: f64,
    pub h: f64,
}
pub fn actor_kind(kind: &str) -> &str {
    if matches!(
        kind,
        "participant" | "actor" | "boundary" | "control" | "entity" | "database" | "collections" | "queue"
    ) {
        kind
    } else {
        "participant"
    }
}
pub fn is_figure(kind: &str) -> bool {
    matches!(kind, "actor" | "boundary" | "control" | "entity" | "database")
}
pub fn actor_size(kind: &str, label: &Label, min_w: f64, min_h: f64, padding_x: f64) -> ActorSize {
    if is_figure(kind) {
        return ActorSize {
            w: 40.0f64.max(label.width + 8.0),
            h: 34.0 + if label.height > 0.0 { 4.0 + label.height } else { 0.0 },
        };
    }
    let extra = if kind == "collections" {
        5.0
    } else if kind == "queue" {
        12.0
    } else {
        0.0
    };
    ActorSize {
        w: min_w.max(label.width + 2.0 * padding_x) + extra,
        h: min_h.max(label.height + 20.0) + if kind == "collections" { 5.0 } else { 0.0 },
    }
}
fn figure(kind: &str, cx: f64, top: f64) -> String {
    let x = num(cx);
    let y = |dy: f64| num(top + dy);
    match kind {
        "actor" => format!(
            "<circle cx=\"{x}\" cy=\"{}\" r=\"6\"{SHAPE}/><path d=\"M{x},{}V{}M{},{}H{}M{},{}L{x},{}L{},{}\"{LINE}/>",
            y(7.0),
            y(13.0),
            y(24.0),
            num(cx - 10.0),
            y(17.0),
            num(cx + 10.0),
            num(cx - 8.0),
            y(34.0),
            y(24.0),
            num(cx + 8.0),
            y(34.0)
        ),
        "boundary" => format!(
            "<path d=\"M{},{}V{}M{},{}H{}\"{LINE}/><circle cx=\"{}\" cy=\"{}\" r=\"11\"{SHAPE}/>",
            num(cx - 19.0),
            y(6.0),
            y(28.0),
            num(cx - 19.0),
            y(17.0),
            num(cx - 8.0),
            num(cx + 3.0),
            y(17.0)
        ),
        "control" => format!(
            "<circle cx=\"{x}\" cy=\"{}\" r=\"11\"{SHAPE}/><path d=\"M{},{}L{},{}L{},{}\"{LINE}/>",
            y(21.0),
            num(cx + 5.0),
            y(5.0),
            num(cx - 1.0),
            y(10.0),
            num(cx + 5.0),
            y(15.0)
        ),
        "entity" => format!(
            "<circle cx=\"{x}\" cy=\"{}\" r=\"11\"{SHAPE}/><path d=\"M{},{}H{}\"{LINE}/>",
            y(15.0),
            num(cx - 11.0),
            y(30.0),
            num(cx + 11.0)
        ),
        _ => {
            let l = num(cx - 14.0);
            let r = num(cx + 14.0);
            format!(
                "<path d=\"M{l},{}V{}A14,5 0 0 0 {r},{}V{}A14,5 0 0 0 {l},{}Z\"{SHAPE}/><path d=\"M{l},{}A14,5 0 0 0 {r},{}\"{LINE}/>",
                y(7.0),
                y(27.0),
                y(27.0),
                y(7.0),
                y(7.0),
                y(7.0),
                y(7.0)
            )
        }
    }
}
pub fn draw_actor(kind: &str, cx: f64, top: f64, size: ActorSize, label: &Label, ink: &str) -> String {
    let ActorSize { w, h } = size;
    let text = " class=\"pele-label\"";
    if is_figure(kind) {
        return figure(kind, cx, top) + &label_svg(label, cx, top + 38.0 + label.height / 2.0, &(text.to_string() + ink));
    }
    let left = cx - w / 2.0;
    if kind == "collections" {
        let bw = w - 5.0;
        let bh = h - 5.0;
        return format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{SHAPE}{ROUNDED}/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{SHAPE}{ROUNDED}/>",
            num(left + 5.0),
            num(top),
            num(bw),
            num(bh),
            num(left),
            num(top + 5.0),
            num(bw),
            num(bh)
        ) + &label_svg(label, left + bw / 2.0, top + 5.0 + bh / 2.0, text);
    }
    if kind == "queue" {
        let l = num(left + 6.0);
        let r = num(left + w - 6.0);
        let b = num(top + h);
        let ry = num(h / 2.0);
        return format!(
            "<path d=\"M{l},{}H{r}A6,{ry} 0 0 1 {r},{b}H{l}A6,{ry} 0 0 1 {l},{}Z\"{SHAPE}/><path d=\"M{r},{}A6,{ry} 0 0 0 {r},{b}\"{LINE}/>",
            num(top),
            num(top),
            num(top)
        ) + &label_svg(label, cx - 3.0, top + h / 2.0, text);
    }
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{SHAPE}{ROUNDED}/>",
        num(left),
        num(top),
        num(w),
        num(h)
    ) + &label_svg(label, cx, top + h / 2.0, text)
}
