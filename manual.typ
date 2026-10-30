// The pele manual. Build it with ./docs.sh, or:
//   typst compile --root . --ignore-system-fonts manual.typ manual.pdf
#import "/lib.typ": mermaid
#import "/docs/style.typ": setup, example, show-function, show-type, parse-functions, accent, rule
#import "/docs/diagrams.typ": families

#let package = toml("/typst.toml").package
#let import-line = "#import \"@preview/pele-rs:" + package.version + "\": mermaid"
#let now = 1768478400000

#show: setup

// Title
#v(3cm)
#align(center)[
  #text(3.2em, weight: "bold", tracking: 0.02em)[pele-rs]
  #v(0.2em)
  #text(1.25em)[Mermaid diagrams in Typst]
  #v(0.6em)
  #text(luma(80))[Version #package.version · #link(package.repository)[#package.repository.trim("https://", at: start)]]
]
#v(1.2cm)
#align(center, box(width: 82%, mermaid(width: 100%, "
flowchart LR
  M[Mermaid text] --> P{{pele_rs}}
  P --> S[(SVG)]
  S --> T([Typst document])
")))
#v(1fr)
#block(stroke: (top: 0.6pt + rule), inset: (top: 0.8em), columns(2, outline(depth: 2, indent: 1em)))
#pagebreak()

= Overview

*pele-rs* draws #link("https://mermaid.js.org")[Mermaid] diagrams in Typst documents. You write the
diagram as text, and it is laid out and drawn while your document compiles: no browser, no
Node.js, no image files to keep in sync.

It is built on `pele_rs`, a Rust port of #link("https://github.com/obsidianmd/pele")[pele], the
fast Mermaid renderer by Obsidian, compiled to a WebAssembly plugin. This version implements all
33 diagram types of pele #raw("1.12.0"), and its output matches the original's byte for byte.
It works in the Typst CLI and in the web app alike. pele-rs is an independent port, not an
official Obsidian or Mermaid project.

#example("#mermaid(\"
sequenceDiagram
  Author->>Typst: compile
  Typst->>pele: render(source)
  pele-->>Typst: SVG
\")")

= Getting started

== Usage

Import the `mermaid` function and call it with the diagram's source:

#raw(block: true, lang: "typ", import-line + "\n\n#mermaid(\"flowchart LR\\n  A --> B --> C\")")

The result is an image, so it can go wherever an image can: in a `figure`, in a `grid`, or
inside `align`, `scale` or `rotate`.

#example("#figure(
  mermaid(\"flowchart LR\\n  A --> B --> C\"),
  caption: [A small flowchart.],
)")

== Code blocks

Longer diagrams are easier to write in a fenced code block. A show rule turns every block
tagged `mermaid` into a diagram:

#example(vertical: true, "#show raw.where(lang: \"mermaid\"): it => mermaid(
  it, font: \"Libertinus Serif\",
)

```mermaid
stateDiagram-v2
  [*] --> Draft
  Draft --> Review: submit
  Review --> Draft: changes
  Review --> [*]: approve
```")

The text of a code block is set in a monospace font, which `mermaid` would otherwise take as
the label font. That is why the rule above names the `font` to use.

== Defaults for a whole document

`mermaid` is a function, not an element, so it cannot be configured with a `set` rule. To use the
same options everywhere, define your own version with `.with`:

```typ
#import "@preview/pele-rs:0.3.0": mermaid
#let mermaid = mermaid.with(
  font: "Libertinus Serif",
  theme: (accent: rgb("#2f5fb3"), radius: 6pt),
  now: 1768478400000,
)
#show raw.where(lang: "mermaid"): mermaid
```

= Basics

== Size

A diagram is drawn at its natural size, with labels at the size of the surrounding text. Like
`image`, `mermaid` takes a `width` and a `height` to scale it, and a `fit` used when both are
given. Extra room is added around the drawing with `padding`.

#example("#mermaid(width: 60%, padding: 0pt,
  \"flowchart LR\\n  A --> B --> C\")")

#example("#mermaid(height: 4em,
  \"flowchart LR\\n  A --> B --> C\")")

== Fonts

Labels use the surrounding text's font and size unless `font` and `font-size` say otherwise.
Some diagrams draw parts of their text in a monospace font, given by `mono-font`; which parts
is chosen with `monospace`.

#example("#mermaid(font: \"DejaVu Sans Mono\", font-size: 9pt,
  monospace: (\"members\",), \"
classDiagram
  class Plugin {
    +render(source) bytes
  }
\")")

Labels are measured with the character widths of Arial, as pele does outside a browser, and
drawn in the font you choose. Fonts of about the same width, or narrower, fit their boxes; a
much wider font can overflow them.

== Themes

Colors and corner radii come from pele's
#link("https://pele.run/theming")[CSS variables]. Pass them in `theme`, by their name without
the `--pele-` prefix. Values may be Typst colors, lengths or strings.

#let theme-keys = (
  ("bg", "#fff", [Background, painted only when set]),
  ("fg", "#1f1f1f", [Text]),
  ("muted", "#6e6e6e", [Secondary text, such as notes and axes]),
  ("line", "#1f1f1f", [Edges and arrows]),
  ("surface", "#f3f3f3", [Node fill]),
  ("surface-alt", "#e6e6e6", [Alternate fill, such as subgraphs and headers]),
  ("border", "#8a8a8a", [Node outlines]),
  ("accent", "#5b7bd5", [Highlights, such as active tasks and selections]),
  ("radius", "4px", [Corner radius of nodes]),
  ("fg-on-light", "#1f1f1f", [Text on light, colored fills]),
  ("fg-on-dark", "#fff", [Text on dark, colored fills]),
  ("critical", "#e45756", [Critical items, such as Gantt's critical tasks]),
  ("bold-weight", "bold", [Weight of bold text]),
  ("title-weight", "bold", [Weight of titles]),
  ("heading-weight", "bold", [Weight of headings]),
)
#block(breakable: false, table(
  columns: (auto, auto, 1fr),
  stroke: none,
  inset: (x: 6pt, y: 4pt),
  table.hline(stroke: 0.8pt),
  table.header([*Key*], [*Default*], [*Used for*]),
  table.hline(stroke: 0.5pt),
  ..theme-keys.map(((key, default, use)) => (raw(key), raw(default), use)).flatten(),
  table.hline(stroke: 0.8pt),
))

Setting `bg` also paints the background behind the diagram; `background` turns this on or off
explicitly.

#example("#mermaid(theme: (
  bg: rgb(\"#1e1e2e\"), fg: rgb(\"#cdd6f4\"),
  line: rgb(\"#89b4fa\"), surface: rgb(\"#313244\"),
  border: rgb(\"#6c7086\"), radius: 6pt,
), \"
flowchart TD
  A[Dark theme] --> B([Rounded])
  A --> C{{Hexagon}}
\")")

== Configuration

Mermaid's configuration options can be set in three places, from lowest to highest
precedence: the `config` argument, front matter at the top of the source, and
`%%{init: ...}%%` directives.

#example("#mermaid(width: 70%,
  config: (pie: (donutHole: 0.5,
    legendPosition: \"bottom\")),
  \"pie\\n  \\\"Rust\\\" : 82\\n  \\\"Typst\\\" : 18\",
)")

#example(vertical: true, "#show raw.where(lang: \"mermaid\"): it => mermaid(it, font: \"Libertinus Serif\")

```mermaid
---
title: Front matter works too
config:
  flowchart:
    curve: linear
---
graph LR
  start([Start]) --> check{OK?}
  check -- yes --> done[[Done]]
  check -- no --> fix[/Fix it/] --> check
```")

== Dates and times

Gantt charts mark today's date and resolve relative dates such as `after a` against the
current time. A Typst document has no clock, so these charts require `now`, a Unix timestamp
in milliseconds; this also makes every build produce the same chart. Calendar arithmetic uses
`time-zone`, an IANA name defaulting to `"UTC"`, including daylight-saving transitions.

#example(vertical: true, "#mermaid(now: 1768478400000, time-zone: \"Europe/Paris\", \"
gantt
  dateFormat YYYY-MM-DD
  section Release
    Write the manual :done, m, 2026-01-12, 3d
    Publish          :active, after m, 4d
\")")

To use the date of the build, compute `now` from `datetime.today()`. In the CLI, setting the
`SOURCE_DATE_EPOCH` environment variable pins that date for reproducible builds.

```typ
#let epoch = datetime(year: 1970, month: 1, day: 1)
#let now = int((datetime.today() - epoch).seconds()) * 1000
```

== Icons and math

pele draws icons written as `fa:fa-name` or `prefix:name` in labels, and math written between
`$$`, with assets you supply. `icons` maps each icon's name to SVG markup drawn in a 24×24
viewport. `math` maps each formula's TeX source to its SVG markup and its size in pixels. Icons
and formulas without an asset fall back to pele's default behavior: formulas are drawn as text.

#example("#mermaid(\"
flowchart LR
  A[\\\"fa:fa-star Starred\\\"] --> B[\\\"$$x^2$$\\\"]
\", icons: (
  \"fa:fa-star\": \"<path d='M12 2l3 7h7l-5.5 4.5 2 7.5-6.5-4.5-6.5 4.5 2-7.5L2 9h7z' fill='currentColor'/>\",
), math: (
  \"x^2\": (svg: \"<svg viewBox='0 0 24 18'><text y='15' font-size='15' font-style='italic'>x²</text></svg>\", width: 24, height: 18),
))")

No icon set or math typesetter is bundled. SVG for math can be produced ahead of time with a
tool such as MathJax; HTML math is not supported.

== Accessibility

Pass a description of the diagram in `alt`. It becomes the image's alternative text in the
PDF, which screen readers announce.

```typ
#mermaid(alt: "Typst sends the source to pele, which returns SVG.", "...")
```

== Errors

Invalid source stops the compilation with pele's message, which gives the line and what was
expected:

#block(fill: luma(247), inset: 10pt, radius: 3pt, width: 100%, text(0.9em, raw(block: true,
"error: plugin errored with: Parse error on line 2:
 A -->
------^
Expecting 'NODE_STRING', got 'EOF'")))

= API reference

#for f in parse-functions(read("/lib.typ")) { show-function(f) }

= Diagram types

The diagram type is chosen by the source's first line. Each example below shows the source on
the left, or below, and what it draws.

#for family in families {
  heading(level: 2, family.name)
  for d in family.diagrams {
    // A diagram's heading and description stay on the page of its example.
    block(breakable: false, width: 100%, {
      heading(level: 3, raw(d.kind))
      block(below: 0.8em, {
        d.desc
        linebreak()
        text(0.85em, luma(90))[Header: #d.headers.map(raw).join([, ])]
      })
      example(d.source, lang: "mermaid", vertical: d.at("wide", default: false),
        options: (now: now))
    })
  }
}

= Compatibility

`pele_rs` #package.version implements the engine of pele #raw("1.12.0"), pinned to commit
#link("https://github.com/obsidianmd/pele/tree/36e584d59152181927e0d8bf70fddcd2a5f33d6c")[`36e584d`].
Given the same source, configuration, text measurement, clock and time zone, it produces the
same SVG, dimensions, links and diagnostics as the original. Its tests compare complete output
for all 522 examples of upstream's documentation, among thousands of other cases.

What a browser adds to pele is outside this package: mounting into a page, measuring text in
the DOM, zooming, clickable links and lazy loading. Typst's SVG renderer does not read CSS, so
the plugin resolves pele's CSS variables, `color-mix()` and `calc()` into plain SVG attributes.

The full contract, including the Rust API and the test suite, is described in
#link(package.repository + "/blob/main/COMPATIBILITY.md")[COMPATIBILITY.md].

pele and `pele_rs` are MIT licensed.
