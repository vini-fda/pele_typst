// Imports the package the way users do, and draws one diagram of each kind it supports.
#import "@local/pele_typst:0.1.0": mermaid

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
