// Imports the package the way users do, and draws one diagram of each kind it supports.
#import "@local/pele_typst:0.3.0": mermaid

#set page(width: 800pt, height: auto, margin: 20pt)
#set text(size: 11pt)
#for diagram in json("diagrams.json") {
  heading(diagram.kind, level: 2)
  mermaid(diagram.source, now: 1768478400000, time-zone: "UTC", width: 100%)
  pagebreak()
}

#mermaid("flowchart LR\n A[\"$$x^2$$ fa:fa-star\"] --> B", icons: (
  "fa:fa-star": "<path d='M2 2H22V22H2Z' fill='currentColor'/>",
), math: (
  "x^2": (svg: "<svg viewBox='0 0 30 18'><path d='M1 1H29V17H1Z' fill='currentColor'/></svg>", width: 30, height: 18),
), theme: (fg: blue), monospace: ("messages",))

#mermaid("gantt\n dateFormat YYYY-MM-DD\n DST :2024-03-09, 3d", now: 1710072000000, time-zone: "America/Los_Angeles")

#mermaid("
flowchart TD
  A[Write Mermaid] --> B{Render with pele}
  B -->|yes| C[(Embed SVG)]
  B -.-> D((Done))
  subgraph Group
    C --> E>Flag]
  end
")

#mermaid(theme: (bg: rgb("#1e1e2e"), fg: white), config: (flowchart: (curve: "linear")), "graph LR\n  a --> b --> c")

#mermaid("
pie showData title Pets adopted by volunteers
  \"Dogs\" : 386
  \"Cats\" : 85
  \"Rats\" : 15
")

#mermaid(config: (pie: (donutHole: 0.5, legendPosition: "bottom")), "pie\n  \"A\" : 1\n  \"B\" : 2")
