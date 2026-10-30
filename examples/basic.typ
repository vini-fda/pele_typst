#import "../lib.typ": mermaid

#set page(width: 16cm, height: auto, margin: 1cm)
#set text(size: 10pt)

= pele

A flowchart drawn by `pele_rs`, inline in Typst:

#mermaid("
flowchart TD
  A[Write Mermaid] --> B[Render with pele]
  B --> C[Embed SVG in Typst]
")

With branches, labels, shapes and a class:

#mermaid("
flowchart LR
  A[Write Mermaid] --> B{Render with pele}
  B -->|yes| C[(Embed SVG)]
  B -.->|no| D((Done))
  C ==> E>Flag]
  classDef hot fill:#f96,stroke:#333
  class D hot
")

Subgraphs, a dark theme and a different font:

#mermaid(
  font: "DejaVu Sans Mono",
  theme: (bg: rgb("#1e1e2e"), fg: rgb("#cdd6f4"), line: rgb("#89b4fa"), surface: rgb("#313244"), surface-alt: rgb("#45475a"), border: rgb("#6c7086")),
  "
flowchart LR
  subgraph Client
    UI[Web app]
    Cache[(Local cache)]
  end
  subgraph Services
    API[API gateway]
    Auth[Auth service]
  end
  UI --> API
  UI --> Cache
  API --> Auth
  Auth -. token .-> UI
",
)

From a raw block, through a show rule:

// Inside a raw block the text font is monospace, so name the font to use.
#show raw.where(lang: "mermaid"): it => mermaid(it, font: "Libertinus Serif")

```mermaid
---
title: Front matter works too
config:
  flowchart:
    curve: linear
---
graph TD
  start([Start]) --> check{OK?}
  check -- yes --> done[[Done]]
  check -- no --> fix[/Fix it/] --> check
```

Pie charts, as a donut with the legend below, at half the page width:

#mermaid(width: 50%, config: (pie: (donutHole: 0.45, legendPosition: "bottom")), "
pie showData title Pets adopted by volunteers
  \"Dogs\" : 386
  \"Cats\" : 85
  \"Rats\" : 15
")
