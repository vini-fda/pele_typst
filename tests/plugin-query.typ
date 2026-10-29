// The runner writes plugin-cases.json before querying this document.
#let wasm = plugin("../pele_typst.wasm")
#for case in json("plugin-cases.json") {
  [#metadata(str(wasm.render(bytes(case.source), bytes(json.encode(case.options))))) <pele-result>]
}
