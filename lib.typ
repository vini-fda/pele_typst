// Renders Mermaid diagrams with pele_rs, a Rust port of pele (https://github.com/obsidianmd/pele),
// compiled to a WebAssembly plugin. Flowcharts and pie charts are supported so far.
//
// Labels are measured with the widths of Arial, as pele does outside a browser, and drawn in the
// font you choose, so a much wider font can overflow its boxes.

#let _plugin = plugin("pele_typst.wasm")

// The colors and sizes of pele's themes, by the short name of each `--pele-*` variable.
#let _theme-keys = (
  "bg", "fg", "muted", "line", "surface", "surface-alt", "border", "accent", "radius",
  "fg-on-light", "fg-on-dark", "critical", "bold-weight", "title-weight", "heading-weight",
)

#let _css(value) = {
  if type(value) == color { value.to-hex() }
  else if type(value) == length { str(value.to-absolute() / 1pt) + "px" }
  else { str(value) }
}

#let _family(fonts) = {
  let list = if type(fonts) == array { fonts } else { (fonts,) }
  list.map(f => if type(f) == dictionary { f.name } else { f }).map(f => "\"" + f + "\"").join(", ")
}

/// Draws a Mermaid diagram.
///
/// - source (str, raw): The Mermaid text, or a raw block holding it.
/// - width (auto, relative): Width of the image; by default the diagram's own size.
/// - height (auto, relative): Height of the image.
/// - fit (str): How the diagram fills a box given both a width and a height.
/// - font (auto, str, array): Font family for labels; by default the surrounding text's.
/// - font-size (auto, length): Size of node labels; by default the surrounding text's.
/// - mono-font (str, array): Font family for monospace label text.
/// - padding (length): Room around the drawing.
/// - theme (dictionary): Colors and sizes by the short name of pele's CSS variables, such as
///   `(fg: black, bg: white, accent: blue, surface: luma(240), radius: 4pt)`.
/// - background (auto, bool): Whether to paint the theme's background behind the diagram; by
///   default, when the theme sets `bg`.
/// - config (dictionary): Mermaid configuration, as in front matter's `config:`, such as
///   `(flowchart: (curve: "linear"))` or `(pie: (donutHole: 0.5))`.
/// - alt (none, str): Alternative text for the image.
#let mermaid(
  source,
  width: auto,
  height: auto,
  fit: "contain",
  font: auto,
  font-size: auto,
  mono-font: "DejaVu Sans Mono",
  padding: 8pt,
  theme: (:),
  background: auto,
  config: (:),
  alt: none,
) = context {
  let text-source = if type(source) == content and source.func() == raw { source.text } else { source }
  assert(type(text-source) == str, message: "mermaid: expected a string or a raw block")
  let vars = (:)
  for (key, value) in theme {
    let name = if key.starts-with("--") { key } else { "--pele-" + key }
    vars.insert(name, _css(value))
  }
  let options = (
    fontSize: (if font-size == auto { text.size } else { font-size }).to-absolute() / 1pt,
    fontFamily: _family(if font == auto { text.font } else { font }),
    fontFamilyMono: _family(mono-font),
    padding: padding.to-absolute() / 1pt,
    theme: vars,
    background: if background == auto { "--pele-bg" in vars } else { background },
    config: config,
  )
  let svg = _plugin.render(bytes(text-source), bytes(json.encode(options)))
  image(svg, format: "svg", width: width, height: height, fit: fit, alt: alt)
}
