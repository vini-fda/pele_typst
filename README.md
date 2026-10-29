# pele_typst

Render [Mermaid](https://mermaid.js.org) diagrams in Typst with **pele_rs**, a Rust port of
[pele](https://github.com/obsidianmd/pele), the fast Mermaid renderer from Obsidian. The port is
compiled to a WebAssembly plugin, so it runs in the Typst CLI and the web app alike.

```typ
#import "@local/pele_typst:0.3.0": mermaid

#mermaid("
flowchart TD
  A[Write Mermaid] --> B[Render with pele]
  B --> C[Embed SVG in Typst]
")
```

Version 0.3.0 implements all **33 diagram types** in **Pele 1.12.0**, pinned to
[`36e584d59152181927e0d8bf70fddcd2a5f33d6c`](https://github.com/obsidianmd/pele/tree/36e584d59152181927e0d8bf70fddcd2a5f33d6c).
The Rust engine matches SVG, dimensions, links, aliases, parsing and diagnostics with identical
inputs, text measurement, clock and timezone. Front matter and `%%{init}%%` configuration are supported.
See the [compatibility contract](COMPATIBILITY.md) for the full type list and Rust API.

## Usage

```typ
#mermaid(
  source,                      // str, or a raw block
  width: auto, height: auto,   // image size; by default the diagram's natural size
  fit: "contain",
  font: auto,                  // label font; by default the surrounding text's
  font-size: auto,             // label size; by default the surrounding text's
  mono-font: "DejaVu Sans Mono",
  monospace: none,             // optional array: members, attributes, files, messages
  now: none,                   // Unix milliseconds; REQUIRED for Gantt
  time-zone: "UTC",            // IANA timezone
  icons: (:),                  // decoded name -> SVG content in a 24×24 viewport
  math: (:),                   // TeX -> (svg: str, width: number, height: number)
  padding: 8pt,
  theme: (:),                  // pele's CSS variables by short name
  background: auto,            // paint theme.bg behind the diagram; on when theme.bg is set
  config: (:),                 // Mermaid config, e.g. (flowchart: (curve: "linear"))
  alt: none,
)
```

Render every `mermaid` raw block:

```typ
#show raw.where(lang: "mermaid"): it => mermaid(it, font: "Libertinus Serif")
```

Inside a raw block the text font is monospace, so pass the font you want.

Theme keys are pele's [CSS variables](https://pele.run/theming) without the `--pele-` prefix:
`bg`, `fg`, `muted`, `line`, `surface`, `surface-alt`, `border`, `accent`, `radius`,
`fg-on-light`, `fg-on-dark`, `critical`, and the `*-weight` keys. Values may be Typst colors,
lengths or strings:

```typ
#mermaid(theme: (bg: rgb("#1e1e2e"), fg: rgb("#cdd6f4"), line: rgb("#89b4fa"),
                 surface: rgb("#313244"), border: rgb("#6c7086"), radius: 6pt), "...")
```

Labels are measured with pele's table of Arial widths and drawn in the font you choose, so a
font much wider than Arial can overflow its boxes.

Gantt rendering is deterministic in Typst: supply a finite Unix timestamp in milliseconds.
Calendar operations use the selected IANA timezone, including daylight-saving transitions.

```typ
#mermaid("gantt\n  dateFormat YYYY-MM-DD\n  Build :2026-01-15, 3d",
  now: 1768478400000, time-zone: "UTC")
```

Icon and math assets are supplied by the caller. Math dimensions are fixed pixels, independent
of label font size; Typst accepts SVG assets and rejects HTML math. Missing assets use Pele's
fallback behavior. No math typesetter is bundled.

```typ
#mermaid("flowchart LR\n A[\"$$x$$ fa:fa-star\"]", icons: (
  "fa:fa-star": "<path d='M2 2H22V22H2Z'/>",
), math: (
  "x": (svg: "<path d='M1 1L17 17M17 1L1 17' stroke='currentColor'/>",
        width: 18, height: 18),
))
```

## Installing

Typst reads local packages from `{data-dir}/typst/packages/local/pele_typst/0.3.0`, where
`{data-dir}` is `~/Library/Application Support` on macOS, `~/.local/share` on Linux and
`%APPDATA%` on Windows. Link or copy this folder there; only `typst.toml`, `lib.typ`,
`pele_typst.wasm`, `README.md`, `COMPATIBILITY.md` and `LICENSE` are needed. For the web app, upload those files to
a project and use "File > Package project" (see
[private packages](https://typst.app/docs/web-app/private-packages/)).

## Building

`pele_typst.wasm` is built from the two Rust crates here:

- `pele_rs/`: the port of pele, a plain Rust library (`pele_rs::render(text, &options)`).
- `plugin/`: the Typst plugin. It renders with `pele_rs` and inlines pele's CSS custom
  properties, `color-mix()` and `calc()` into plain SVG attributes, which Typst's SVG renderer needs.

```sh
rustup target add wasm32-unknown-unknown
./build.sh
```

## Tests

The oracle includes all 522 upstream documentation examples, all 239 agentflow reference
examples, existing fixtures, upstream test inputs, and explicit option, asset and error cases.
Comparisons include complete SVG, dimensions, type aliases, links, resolver calls and structured
errors. Successful models are reused at default, 320, 500, then default width. Calendar tests
add 7,118 date, scheduling and rendering cases in UTC and America/Los_Angeles.
The [coverage manifest](pele_rs/tests/fixtures/coverage.json) records per-type totals.

Regenerate fixtures with [Bun](https://bun.sh) from the pinned checkout:

```sh
bun pele_rs/scripts/fixtures.ts /path/to/pinned/pele --check
bun pele_rs/scripts/gantt-calendar.ts /path/to/pinned/pele --check
bun pele_rs/scripts/agentflow-model.ts /path/to/pinned/pele --check
cargo test --workspace
cargo build --workspace --examples
bun pele_rs/scripts/fuzz.ts /path/to/pinned/pele 40
python3 tests/plugin-parity.py --typst typst
```

CI checks formatting, native/WASM Clippy, oracle reproducibility, native/WASM SVG parity and
package smoke documents using both the committed and rebuilt plugin on Typst 0.13 and latest.

## License

MIT. pele is MIT licensed by kepano; see [LICENSE](LICENSE).
