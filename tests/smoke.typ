// Imports the package the way users do, and draws one diagram of each kind it supports.
#import "@local/pele_typst:0.2.0": mermaid

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
