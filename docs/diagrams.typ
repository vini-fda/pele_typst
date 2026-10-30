// One example of each diagram type, grouped as in COMPATIBILITY.md. `wide` examples are drawn
// above their source instead of beside it.

#let families = (
  (
    name: "Flow and charts",
    diagrams: (
      (
        kind: "flowchart",
        headers: ("flowchart", "graph", "flowchart-elk"),
        desc: [Nodes in a dozen shapes, joined by solid, dotted or thick links, in subgraphs.],
        wide: true,
        source: "flowchart LR
  A[Write Mermaid] --> B{Valid?}
  B -->|yes| C[(Render)]
  B -.->|no| D>Fix the syntax]
  D --> A
  subgraph Typst
    C --> E((PDF))
  end",
      ),
      (
        kind: "pie",
        headers: ("pie",),
        desc: [Proportions of a whole, optionally as a donut.],
        source: "pie showData
  title Pets adopted
  \"Dogs\" : 386
  \"Cats\" : 85
  \"Rats\" : 15",
      ),
      (
        kind: "info",
        headers: ("info",),
        desc: [The version of pele this package is compatible with.],
        source: "info",
      ),
      (
        kind: "packet",
        headers: ("packet", "packet-beta"),
        desc: [The bit layout of a network packet or binary format.],
        wide: true,
        source: "packet
title UDP Packet
+16: \"Source Port\"
+16: \"Destination Port\"
32-47: \"Length\"
48-63: \"Checksum\"
64-95: \"Data (variable length)\"",
      ),
      (
        kind: "quadrantChart",
        headers: ("quadrantChart",),
        desc: [Points placed on two axes, in four labelled quadrants.],
        source: "quadrantChart
  title Diagram choice
  x-axis Simple --> Detailed
  y-axis Static --> Dynamic
  quadrant-1 Sequence
  quadrant-2 Timeline
  quadrant-3 Pie
  quadrant-4 Class
  Flowchart: [0.45, 0.6]
  Gantt: [0.7, 0.75]
  Mindmap: [0.3, 0.25]",
      ),
      (
        kind: "radar",
        headers: ("radar-beta",),
        desc: [Several series compared on radial axes.],
        source: "radar-beta
  title Restaurants
  axis food[\"Food\"], service[\"Service\"]
  axis price[\"Price\"], mood[\"Ambiance\"]
  curve a[\"Bistro\"]{4, 3, 2, 4}
  curve b[\"Diner\"]{3, 4, 3, 3}
  max 5",
      ),
      (
        kind: "xychart",
        headers: ("xychart", "xychart-beta"),
        desc: [Bar and line series on categorical or numeric axes.],
        source: "xychart
  title \"Render time\"
  x-axis [flow, seq, gantt, mind]
  y-axis \"ms\" 0 --> 10
  bar [4, 3, 6, 5]
  line [4, 3, 6, 5]",
      ),
      (
        kind: "cynefin",
        headers: ("cynefin-beta",),
        desc: [The Cynefin sense-making framework, with items and transitions.],
        wide: true,
        source: "cynefin-beta
  title Team Practices
  complex
    \"Retrospectives\"
  complicated
    \"Code review\"
  clear
    \"Deployment checklist\"
  chaotic
    \"Incident response\"
  complex --> complicated : \"Pattern emerges\"
  chaotic --> complex : \"Stabilized\"",
      ),
    ),
  ),
  (
    name: "Structured layouts",
    diagrams: (
      (
        kind: "mindmap",
        headers: ("mindmap",),
        desc: [A tree of ideas around a central topic, given by indentation.],
        wide: true,
        source: "mindmap
  root((pele))
    Engine
      Parsing
      Layout
      SVG
    Package
      lib.typ
      WASM plugin
    Tests",
      ),
      (
        kind: "kanban",
        headers: ("kanban",),
        desc: [Columns of task cards.],
        wide: true,
        source: "kanban
  todo[To do]
    docs[Write the manual]
  doing[In progress]
    themes[More themes]
  done[Done]
    all[All 33 types]",
      ),
      (
        kind: "treemap",
        headers: ("treemap", "treemap-beta"),
        desc: [Nested rectangles sized by value.],
        source: "treemap-beta
\"Budget\"
  \"Operations\"
    \"Salaries\": 70
    \"Equipment\": 20
  \"Marketing\"
    \"Ads\": 40
    \"Events\": 10",
      ),
      (
        kind: "treeView",
        headers: ("treeView-beta",),
        desc: [A file or folder hierarchy.],
        source: "treeView-beta
  src/
    lib.rs
    detect.rs
  plugin/
    lib.rs
  lib.typ
  typst.toml",
      ),
      (
        kind: "timeline",
        headers: ("timeline",),
        desc: [Events grouped by period.],
        wide: true,
        source: "timeline
  title pele for Typst
  0.1.0 : Flowcharts
  0.2.0 : Pie charts
  0.3.0 : All 33 diagram types",
      ),
      (
        kind: "journey",
        headers: ("journey",),
        desc: [The steps of a user journey, scored by satisfaction.],
        wide: true,
        source: "journey
  title Adding a diagram to a paper
  section Write
    Draft in Mermaid: 4: Author
    Preview in Typst: 5: Author
  section Review
    Read the PDF: 5: Reviewer",
      ),
      (
        kind: "ishikawa",
        headers: ("ishikawa", "ishikawa-beta"),
        desc: [A fishbone diagram of the causes of an effect.],
        wide: true,
        source: "ishikawa-beta
  Blurry Photo
  Process
    Out of focus
    Shutter too slow
  User
    Shaky hands
  Equipment
    Dirty lens
  Environment
    Too dark",
      ),
      (
        kind: "eventmodeling",
        headers: ("eventmodeling",),
        desc: [Screens, commands and events laid out on a timeline.],
        wide: true,
        source: "eventmodeling

tf 01 ui CartUI
tf 02 cmd AddItem [[AddItem]]
tf 03 evt ItemAdded [[ItemAdded]]

data AddItem {
  description: 'john'
  price: 20.4
}

data ItemAdded {
  description: string
  price: number
}",
      ),
      (
        kind: "wardley",
        headers: ("wardley-beta",),
        desc: [A Wardley map of a value chain against evolution.],
        wide: true,
        source: "wardley-beta
title Tea Shop

anchor Business [0.95, 0.63]
component Cup of Tea [0.79, 0.61]
component Tea [0.63, 0.81]
component Hot Water [0.52, 0.80]
component Kettle [0.43, 0.35]
component Power [0.10, 0.70]

Business -> Cup of Tea
Cup of Tea -> Tea
Cup of Tea -> Hot Water
Hot Water -> Kettle
Kettle -> Power

evolve Kettle 0.62",
      ),
    ),
  ),
  (
    name: "Graphs",
    diagrams: (
      (
        kind: "er",
        headers: ("erDiagram",),
        desc: [Entities and the cardinality of their relationships.],
        source: "erDiagram
  AUTHOR ||--o{ DOCUMENT : writes
  DOCUMENT ||--|{ DIAGRAM : contains
  DIAGRAM }o--|| THEME : uses",
      ),
      (
        kind: "class",
        headers: ("classDiagram", "classDiagram-v2"),
        desc: [UML classes, their members and relationships.],
        source: "classDiagram
  class Diagram {
    +String kind
    +parse(source) Model
    +render(options) Svg
  }
  Diagram <|-- Flowchart
  Diagram <|-- Pie",
      ),
      (
        kind: "state",
        headers: ("stateDiagram", "stateDiagram-v2"),
        desc: [UML state machines, with composite states and notes.],
        source: "stateDiagram-v2
  [*] --> Draft
  Draft --> Review: submit
  Review --> Draft: changes
  Review --> Published: approve
  Published --> [*]",
      ),
      (
        kind: "requirement",
        headers: ("requirementDiagram",),
        desc: [SysML requirements and the elements that satisfy them.],
        source: "requirementDiagram
  requirement checkout {
    id: 1
    text: Orders are payable online.
    risk: high
    verifymethod: test
  }
  element payment_service {
    type: service
  }
  payment_service - satisfies -> checkout",
      ),
      (
        kind: "usecase",
        headers: ("usecase-beta",),
        desc: [UML use cases, actors and system boundaries.],
        source: "usecase-beta
actor Customer
systemBoundary Shop
  Browse[Browse catalogue]
  Checkout[Check out]
end
Customer --> Browse
Customer --> Checkout",
      ),
      (
        kind: "architecture",
        headers: ("architecture-beta",),
        desc: [Services and resources in groups, joined edge to edge.],
        source: "architecture-beta
  group api(cloud)[API]
  service db(database)[Database] in api
  service disk(disk)[Storage] in api
  service server(server)[Server] in api
  db:L -- R:server
  disk:T -- B:server",
      ),
      (
        kind: "c4",
        headers: ("C4Context", "C4Container", "C4Component", "C4Dynamic", "C4Deployment"),
        desc: [C4 model diagrams of software architecture.],
        wide: true,
        source: "C4Context
  title System context
  Person(user, \"Author\", \"Writes documents\")
  System(typst, \"Typst\", \"Compiles the document\")
  System_Ext(pele, \"pele\", \"Draws the diagrams\")
  Rel(user, typst, \"Writes\")
  Rel(typst, pele, \"Renders with\")",
      ),
      (
        kind: "block",
        headers: ("block", "block-beta"),
        desc: [Blocks placed on a grid of columns, under your control.],
        wide: true,
        source: "block
  columns 3
  Frontend arrow<[\" \"]>(right) Backend
  space:2 down<[\" \"]>(down)
  Disk left<[\" \"]>(left) Database[(\"Database\")]",
      ),
      (
        kind: "agentflow",
        headers: ("agentflow-beta",),
        desc: [AI agent workflows: tasks, tools, decisions and the agents running them.],
        wide: true,
        source: "agentflow-beta LR
  brief[\"Release brief\"]@{ shape: input }
  flow writer[\"Drafting Agent\"]
    draft[\"Draft the notes\"]@{ shape: task }
    lookup[\"changelog_search\"]@{ shape: tool }
    draft --> lookup
  end
  publish[\"Publish\"]@{ shape: action }
  brief --> writer
  writer --> publish",
      ),
      (
        kind: "swimlane",
        headers: ("swimlane-beta",),
        desc: [A process split into lanes, one per participant.],
        wide: true,
        source: "swimlane-beta LR
  subgraph Customer
    request[Request service]
    receive[Receive update]
  end
  subgraph Support
    triage[Triage request]
    answer[Send answer]
  end
  request --> triage
  triage --> answer
  answer --> receive",
      ),
    ),
  ),
  (
    name: "Specialized engines",
    diagrams: (
      (
        kind: "sequence",
        headers: ("sequenceDiagram",),
        desc: [Messages exchanged between participants over time.],
        wide: true,
        source: "sequenceDiagram
  participant T as Typst
  participant P as Plugin
  participant R as pele_rs
  T->>P: render(source, options)
  P->>R: parse and lay out
  R-->>P: SVG
  Note over P: inline CSS variables
  P-->>T: SVG bytes",
      ),
      (
        kind: "gantt",
        headers: ("gantt",),
        desc: [Tasks scheduled on a calendar. Requires `now`.],
        wide: true,
        source: "gantt
  title Release plan
  dateFormat YYYY-MM-DD
  section Engine
    Port flowcharts :done, f, 2026-01-05, 7d
    Port pie charts :done, p, after f, 3d
    Port the rest   :active, r, after p, 14d
  section Package
    Publish 0.3.0   :milestone, after r, 0d",
      ),
      (
        kind: "gitGraph",
        headers: ("gitGraph",),
        desc: [Commits, branches and merges of a Git history.],
        wide: true,
        source: "gitGraph
  commit id: \"flowcharts\"
  branch pie
  commit id: \"pie charts\"
  checkout main
  merge pie
  commit id: \"0.3.0\"",
      ),
      (
        kind: "sankey",
        headers: ("sankey", "sankey-beta"),
        desc: [Flows between nodes, drawn with proportional widths.],
        source: "sankey
Sources,Parser,40
Sources,Front matter,10
Parser,Layout,40
Front matter,Layout,10
Layout,SVG,50",
      ),
      (
        kind: "venn",
        headers: ("venn-beta",),
        desc: [Overlapping sets and their unions.],
        source: "venn-beta
  set Desirable
  set Feasible
  set Viable
  union Desirable,Feasible[\"Buildable\"]
  union Desirable,Feasible,Viable[\"Ship it\"]",
      ),
      (
        kind: "railroad",
        headers: ("railroad-beta", "railroad-ebnf-beta", "railroad-abnf-beta", "railroad-peg-beta"),
        desc: [Syntax diagrams of a grammar, in EBNF, ABNF, PEG or pele's own notation.],
        wide: true,
        source: "railroad-ebnf-beta
title \"Expressions\"
expression = term ( \"+\" term | \"-\" term )* ;
term = factor ( \"*\" factor | \"/\" factor )* ;
factor = number | \"(\" expression \")\" ;",
      ),
    ),
  ),
)
