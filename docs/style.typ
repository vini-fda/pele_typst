// Helpers for manual.typ: type pills, the API reference read from lib.typ's doc comments, and
// boxes that show an example's output beside its source.

#import "/lib.typ": mermaid

#let accent = rgb("#2f5fb3")
#let rule = luma(200)

#let type-colors = (
  "content": rgb("#a6ebe6"),
  "raw": rgb("#a6ebe6"),
  "str": rgb("#d1ffe2"),
  "none": rgb("#ffcbc4"),
  "auto": rgb("#ffcbc4"),
  "bool": rgb("#ffedc1"),
  "int": rgb("#e7d9ff"),
  "float": rgb("#e7d9ff"),
  "length": rgb("#e7d9ff"),
  "relative": rgb("#e7d9ff"),
  "dictionary": rgb("#eff0f3"),
  "array": rgb("#eff0f3"),
)

#let show-type(name) = box(
  raw(name),
  inset: 2pt,
  baseline: 2pt,
  radius: 2pt,
  fill: type-colors.at(name, default: luma(239)),
)

// Splits `text` at the commas that are not inside brackets or strings.
#let _split-top-level(text) = {
  let (parts, current, depth, quoted) = ((), "", 0, false)
  for c in text.clusters() {
    if c == "\"" { quoted = not quoted }
    if not quoted and c in ("(", "[", "{") { depth += 1 }
    if not quoted and c in (")", "]", "}") { depth -= 1 }
    if c == "," and depth == 0 and not quoted {
      parts.push(current)
      current = ""
    } else {
      current += c
    }
  }
  parts.push(current)
  parts.map(str.trim).filter(p => p != "")
}

// Reads the doc comment and signature of every public `#let name(...) =` in `source`.
#let parse-functions(source) = {
  let lines = source.split("\n")
  let functions = ()
  for (i, line) in lines.enumerate() {
    let head = line.match(regex("^#let ([\w-]+)\("))
    if head == none or head.captures.first().starts-with("_") { continue }

    // The signature runs to the line holding `) =`.
    let signature = line.slice(head.end)
    let j = i
    while not lines.at(j).contains(") =") {
      j += 1
      signature += lines.at(j)
    }
    signature = signature.slice(0, signature.position(") ="))
    let params = _split-top-level(signature).map(p => {
      let at = p.position(":")
      if at == none { (name: p, default: none) } else {
        (name: p.slice(0, at).trim(), default: p.slice(at + 1).trim())
      }
    })

    // The doc comment is the run of `///` lines just above.
    let k = i
    while k > 0 and lines.at(k - 1).starts-with("///") { k -= 1 }
    let doc = lines.slice(k, i).map(l => l.slice(if l.starts-with("/// ") { 4 } else { 3 }))

    let (description, docs, returns) = ((), (:), none)
    let current = none
    for l in doc {
      let param = l.match(regex("^- ([\w-]+) \(([^)]*)\): (.*)$"))
      let ret = l.match(regex("^-> (.*)$"))
      if param != none {
        let (name, types, text) = param.captures
        current = name
        docs.insert(name, (types: types.split(",").map(str.trim), text: text))
      } else if ret != none {
        returns = ret.captures.first().split("|").map(str.trim)
      } else if current != none and l.starts-with("  ") {
        docs.at(current).text += " " + l.trim()
      } else {
        current = none
        description.push(l)
      }
    }

    functions.push((
      name: head.captures.first(),
      description: description.join("\n").trim(),
      params: params.map(p => p + docs.at(p.name, default: (types: ("any",), text: ""))),
      returns: returns,
    ))
  }
  functions
}

#let _markup(text) = eval(text, mode: "markup", scope: (mermaid: mermaid))

// The signature, description and parameters of one function, in the style of the cetz manual.
#let show-function(f) = {
  [#heading(level: 2, raw(f.name)) #label(f.name)]

  block(fill: luma(247), inset: 10pt, radius: 3pt, width: 100%, {
    set par(leading: 0.45em)
    set text(size: 0.95em)
    text(accent, raw(f.name)) + raw("(") + linebreak()
    f.params.map(p => {
      h(1.5em) + raw(if p.default == none { p.name } else { p.name + ":" }) + [ ]
      p.types.map(show-type).join([ ])
    }).join(raw(",") + linebreak())
    linebreak() + raw(")")
    if f.returns != none { [ #sym.arrow.r ] + f.returns.map(show-type).join([ ]) }
  })

  _markup(f.description)

  heading(level: 3, outlined: false, numbering: none)[Parameters]
  for p in f.params {
    block(breakable: false, above: 1.1em, {
      strong(raw(p.name)) + h(0.6em) + p.types.map(show-type).join([ ])
      if p.default != none {
        h(1fr) + text(0.85em, luma(90))[Default: #raw(p.default, lang: "typc")]
      }
      block(above: 0.55em, inset: (left: 1.2em), _markup(p.text))
    })
  }
}

// An example box: what `code` produces, beside or above the code itself. `code` is Typst markup
// with `mermaid` in scope, or Mermaid text when `lang` is "mermaid".
#let example(code, lang: "typ", vertical: false, options: (:)) = {
  let output = if lang == "mermaid" {
    mermaid(code, ..options)
  } else {
    _markup(code)
  }
  let source = block(width: 100%, {
    set text(0.85em)
    raw(code, lang: if lang == "mermaid" { none } else { lang }, block: true)
  })
  let stroke = (paint: rule, thickness: 0.6pt, dash: "dashed")
  block(stroke: 0.8pt + rule, radius: 3pt, breakable: false, width: 100%, if vertical {
    grid(
      columns: 1fr,
      inset: 10pt,
      align(center, output),
      grid.hline(stroke: stroke),
      source,
    )
  } else {
    grid(
      columns: (1fr, 1fr),
      inset: 10pt,
      align: (left + top, center + horizon),
      source,
      grid.vline(stroke: stroke),
      output,
    )
  })
}

#let setup(body) = {
  set document(title: "pele-rs manual", author: "Vini Freitas")
  set page(paper: "a4", margin: (x: 2.2cm, y: 2.4cm), numbering: "1", number-align: center)
  set text(font: "Libertinus Serif", size: 10.5pt, lang: "en")
  set par(justify: true)
  show raw: set text(font: "DejaVu Sans Mono", size: 1.08em)
  show link: set text(accent)

  set heading(numbering: (..nums) => {
    let nums = nums.pos()
    if nums.len() <= 2 { numbering("1.1", ..nums) }
  })
  show heading: set block(above: 1.6em, below: 0.9em)
  show heading.where(level: 1, outlined: true): it => {
    pagebreak(weak: true)
    set text(1.5em)
    block(below: 1.1em, it)
  }

  show raw.where(lang: "example"): it => example(it.text)
  show raw.where(lang: "example-vertical"): it => example(it.text, vertical: true)

  body
}
