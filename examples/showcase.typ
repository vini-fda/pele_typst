// A tour of pele-rs 0.3.0: one diagram from each family, written the ways you would use it.
// Compile with: typst compile examples/showcase.typ
#import "@local/pele-rs:0.3.0": mermaid

#set page(paper: "a4", margin: 2cm, numbering: "1")
#set text(font: "Libertinus Serif", size: 10.5pt)
#set heading(numbering: "1.")
#show heading.where(level: 1): set block(above: 1.6em, below: 0.9em)
#show figure: set block(breakable: false)

// Every ```mermaid block becomes a diagram. A raw block's text is monospace, so the label font
// is set here. Gantt charts need a fixed clock, so `now` is set too (15 January 2026, 12:00 UTC).
#show raw.where(lang: "mermaid"): it => align(center, mermaid(
  it,
  font: "Libertinus Serif",
  font-size: 10pt,
  now: 1768478400000,
  time-zone: "UTC",
))

#align(center, text(18pt, weight: "bold")[pele showcase])
#align(center, text(fill: luma(90))[Mermaid diagrams rendered by a Rust port of pele, inside Typst])

#outline(depth: 1)

= Flowcharts and graphs

The plain call takes a string. Width and height scale the diagram like any image.

#figure(
  mermaid("
flowchart LR
  A[Write Mermaid] --> B{Valid?}
  B -->|yes| C[(Render with pele)]
  B -.->|no| D>Fix the syntax]
  D --> A
  subgraph Typst
    C --> E((PDF))
  end
", width: 85%),
  caption: [A flowchart passed as a string.],
)

With the show rule above, a fenced block is enough:

```mermaid
stateDiagram-v2
  [*] --> Draft
  Draft --> Review: submit
  Review --> Draft: changes requested
  Review --> Published: approve
  Published --> [*]
```

```mermaid
classDiagram
  class Diagram {
    +String kind
    +parse(source) Model
    +render(options) Svg
  }
  class Flowchart
  class Pie
  Diagram <|-- Flowchart
  Diagram <|-- Pie
```

```mermaid
erDiagram
  AUTHOR ||--o{ DOCUMENT : writes
  DOCUMENT ||--|{ DIAGRAM : contains
  DIAGRAM }o--|| THEME : uses
```

= Sequences and timelines

```mermaid
sequenceDiagram
  participant T as Typst
  participant P as Plugin (WASM)
  participant R as pele_rs
  T->>P: render(source, options)
  P->>R: parse and lay out
  R-->>P: SVG
  Note over P: inline CSS variables
  P-->>T: SVG bytes
```

```mermaid
gantt
  title Release plan
  dateFormat YYYY-MM-DD
  section Engine
    Port flowcharts      :done,   f, 2026-01-05, 7d
    Port pie charts      :done,   p, after f, 3d
    Port remaining types :active, r, after p, 14d
  section Package
    Publish 0.3.0        :milestone, after r, 0d
```

```mermaid
timeline
  title pele
  0.1.0 : Flowcharts
  0.2.0 : Pie charts
  0.3.0 : All 33 diagram types
```

```mermaid
gitGraph
  commit id: "flowcharts"
  branch add-pie-charts
  commit id: "pie"
  checkout main
  merge add-pie-charts
  commit id: "0.3.0"
```

= Charts

#grid(
  columns: (1fr, 1fr),
  gutter: 1em,
  align: center + horizon,
  mermaid("
pie showData title Diagrams by family
  \"Flow and charts\" : 8
  \"Structured\" : 9
  \"Graphs\" : 10
  \"Specialized\" : 6
", width: 100%),
  mermaid(config: (pie: (donutHole: 0.55, legendPosition: "bottom")), "
pie
  \"Rust\" : 82
  \"Typst\" : 6
  \"TypeScript\" : 12
", width: 70%),
)

Diagrams that are large at their natural size can be scaled with `width` or `height`:

#align(center, mermaid(width: 75%, "
xychart
  title \"Render time (ms)\"
  x-axis [flowchart, sequence, gantt, mindmap, sankey]
  y-axis \"milliseconds\" 0 --> 20
  bar [4, 3, 6, 5, 8]
  line [4, 3, 6, 5, 8]
"))

```mermaid
quadrantChart
  title Diagram choice
  x-axis Simple --> Detailed
  y-axis Static --> Dynamic
  quadrant-1 Sequence
  quadrant-2 Timeline
  quadrant-3 Pie
  quadrant-4 Class
  Flowchart: [0.45, 0.6]
  Gantt: [0.7, 0.75]
  Mindmap: [0.3, 0.25]
```

#align(center, mermaid(height: 6cm, "
sankey
Sources,Parser,40
Sources,Front matter,10
Parser,Layout,40
Front matter,Layout,10
Layout,SVG,50
"))

= Structured layouts

```mermaid
mindmap
  root((pele))
    Engine
      Parsing
      Layout
      SVG
    Package
      lib.typ
      WASM plugin
    Tests
      Fixtures
      CI
```

```mermaid
journey
  title Adding a diagram to a paper
  section Write
    Draft in Mermaid: 4: Author
    Preview in Typst: 5: Author
  section Review
    Read the PDF: 5: Reviewer
```

```mermaid
kanban
  todo[To do]
    docs[Write the manual]
  doing[In progress]
    themes[More themes]
  done[Done]
    all[All 33 types]
```

= Theming

Theme keys are pele's CSS variables without the `--pele-` prefix. Setting `bg` paints the
background too.

#figure(
  mermaid(
    "flowchart TD\n  A[Dark theme] --> B([Rounded])\n  A --> C{{Hexagon}}\n  B & C --> D[(Store)]",
    theme: (
      bg: rgb("#1e1e2e"),
      fg: rgb("#cdd6f4"),
      line: rgb("#89b4fa"),
      surface: rgb("#313244"),
      border: rgb("#6c7086"),
      accent: rgb("#f38ba8"),
      radius: 6pt,
    ),
    config: (flowchart: (curve: "linear")),
    width: 55%,
  ),
  caption: [Catppuccin-like colors and straight edges.],
)
