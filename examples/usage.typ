#import "@local/pele:0.3.0": mermaid

// 1. Pass the diagram as a string
#mermaid("flowchart LR\n  A --> B --> C", width: 80%)

// 2. Or make every mermaid code block a diagram
#show raw.where(lang: "mermaid"): it => mermaid(
  it,
  font: "Libertinus Serif",   // code blocks default to a monospace font, so set one
  now: 1768478400000,         // fixed clock in Unix ms; Gantt charts require it
)

```mermaid
sequenceDiagram
  Alice->>Bob: Hello
```

// 3. Themes and Mermaid settings
#mermaid(
  "graph TD\n  A --> B",
  theme: (bg: rgb("#1e1e2e"), fg: white, line: rgb("#89b4fa"), radius: 6pt),
  config: (flowchart: (curve: "linear")),
)
