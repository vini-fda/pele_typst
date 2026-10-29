pub fn resolve(shape: &str) -> &str {
    match shape {
        "task" => "roundedRect",
        "tool" => "subroutine",
        "input" => "lean-right",
        "decision" => "diamond",
        "refdoc" => "lin-doc",
        "action" => "hexagon",
        "round" => "rect",
        _ => shape,
    }
}
pub fn valid(shape: &str) -> bool {
    crate::diagrams::flowchart::shapes::is_valid_shape(shape) || shape == "constructor"
}
pub fn removed(shape: &str) -> bool {
    matches!(
        shape,
        "doc"
            | "stadium"
            | "terminal"
            | "circle"
            | "trapezoid"
            | "inv_trapezoid"
            | "inv-trapezoid"
            | "doublecircle"
            | "double-circle"
            | "typeDeclaration"
            | "procs"
            | "lean_left"
            | "lean-left"
            | "in-out"
            | "cylinder"
            | "ellipse"
            | "odd"
            | "tag-rect"
            | "tagged-rectangle"
            | "delay"
            | "half-rounded-rectangle"
            | "lin-rect"
            | "lined-rectangle"
            | "win-pane"
            | "window-pane"
            | "curv-trap"
            | "curved-trapezoid"
    )
}
pub fn allowed(shape: &str) -> bool {
    matches!(
        shape,
        "roundedRect"
            | "subroutine"
            | "subprocess"
            | "subproc"
            | "framed-rectangle"
            | "lean-right"
            | "diamond"
            | "lin-doc"
            | "lined-document"
            | "hexagon"
            | "hex"
            | "connector"
            | "collapsedGroup"
    )
}
pub fn kind_slot(kind: &str) -> Option<usize> {
    match kind {
        "tool" => Some(0),
        "task" => Some(1),
        "decision" => Some(2),
        "input" => Some(3),
        "refdoc" => Some(4),
        "connector" => Some(5),
        "action" => Some(6),
        _ => None,
    }
}
