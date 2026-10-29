#!/usr/bin/env python3
"""Compare the Typst WASM ABI with native inlined SVG for every kind and alias."""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--typst", default="typst")
parser.add_argument("--native", default=str(ROOT / "target/debug/examples/bridge"))
args = parser.parse_args()
cases = []
for diagram in json.loads((ROOT / "tests/diagrams.json").read_text()):
    for extra in [{}, {"fontSize": 20, "fontFamily": "DejaVu Sans", "fontFamilyMono": "DejaVu Sans Mono", "padding": 12, "maxWidth": 320,
                       "monospace": ["members", "attributes", "files", "messages"], "theme": {"--pele-bg": "#182333", "--pele-fg": "#eeeeee"}, "background": True}]:
        cases.append({"source": diagram["source"], "options": {"now": 1768478400000, "timeZone": "UTC", **extra}})
cases.extend([
    {"source": 'flowchart LR\n A["$$x^2$$ fa:fa-star"] --> B', "options": {
        "icons": {"fa:fa-star": "<svg viewBox='0 0 24 24'><defs><linearGradient id='asset-gradient'><stop stop-color='currentColor'/></linearGradient></defs><path d='M2 2H22V22H2Z' fill='url(#asset-gradient)'/></svg>"},
        "math": {"x^2": {"svg": "<path d='M1 1H29' stroke='currentColor'/>", "width": 30, "height": 18}},
        "theme": {"--pele-fg": "#2244aa"}}},
    {"source": "gantt\n dateFormat YYYY-MM-DD\n DST :2024-03-09, 3d", "options": {"now": 1710072000000, "timeZone": "America/Los_Angeles"}},
    {"source": "flowchart LR\n A --> B", "options": {"theme": {"--pele-bg": "hsl(240 100% 50% / .5)", "--pele-fg": "rebeccapurple", "--pele-surface": "papayawhip"}, "background": True}},
])
native = subprocess.run([args.native], input="".join(json.dumps(c) + "\n" for c in cases), text=True, capture_output=True, check=True)
expected = [json.loads(line) for line in native.stdout.splitlines()]
assert len(expected) == len(cases)
for i, result in enumerate(expected):
    assert "svg" in result, (i, result)
path = ROOT / "tests/plugin-cases.json"
try:
    path.write_text(json.dumps(cases))
    result = subprocess.run([args.typst, "query", "--root", str(ROOT), "--format", "json", "--field", "value", str(ROOT / "tests/plugin-query.typ"), "<pele-result>"], text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr)
    actual = json.loads(result.stdout)
    assert len(actual) == len(expected), (len(actual), len(expected))
    for i, (native_result, wasm) in enumerate(zip(expected, actual)):
        if native_result["svg"] != wasm:
            (ROOT / "target/native-mismatch.svg").write_text(native_result["svg"])
            (ROOT / "target/wasm-mismatch.svg").write_text(wasm)
            raise AssertionError(f"native/WASM mismatch at case {i}: {cases[i]['source'][:100]}")
finally:
    path.unlink(missing_ok=True)
print(f"{len(cases)} native/WASM SVG comparisons passed ({args.typst})")
