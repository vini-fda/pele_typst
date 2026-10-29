// Renders Mermaid sources with pele's JavaScript and writes what it gives, for tests/fixtures.rs
// to compare the Rust port against. Run with bun:
//
//   bun scripts/fixtures.ts <path to a pele checkout>
//
// For each ported type, the sources are pele's docs corpus, every string literal in pele's tests
// that is a diagram of that type, and the cases in scripts/extra.
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const TYPES = ['flowchart', 'pie'];

const pele = process.argv[2];
if (!pele) throw new Error('usage: bun scripts/fixtures.ts <path to a pele checkout>');
const { render, detectType } = await import(join(pele, 'src/index.ts'));

const literals = new Set<string>();
const walk = (dir: string): void => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (path.endsWith('.ts')) harvest(readFileSync(path, 'utf8'));
  }
};
const LITERAL = /`((?:[^`\\$]|\\.|\$(?!\{))*)`|'((?:[^'\\\n]|\\.)*)'|"((?:[^"\\\n]|\\.)*)"/g;
function harvest(text: string): void {
  for (const m of text.matchAll(LITERAL)) {
    try {
      const value = m[1] !== undefined ? eval('`' + m[1] + '`') : eval((m[2] !== undefined ? "'" + m[2] + "'" : '"' + m[3] + '"'));
      if (typeof value === 'string' && value.includes('\n')) literals.add(value);
    } catch {
      // Not a plain literal.
    }
  }
}
walk(join(pele, 'tests'));

for (const type of TYPES) {
  const read = (path: string): string[] => (existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : []);
  const sources = new Set<string>([
    ...read(join(pele, `tests/corpus/${type}-docs.json`)),
    ...read(join(import.meta.dir, `extra/${type}.json`)),
  ]);
  for (const value of literals) {
    let kind: string | null = null;
    try {
      kind = detectType(value);
    } catch {
      continue;
    }
    if (kind === type) sources.add(value);
  }
  const cases = [...sources].map((source) => {
    try {
      return { source, svg: render(source).svg };
    } catch (error) {
      return { source, error: (error as Error).message };
    }
  });
  writeFileSync(join(import.meta.dir, `../tests/fixtures/${type}.json`), JSON.stringify(cases, null, 1));
  console.log(`${type}: ${cases.length} cases, ${cases.filter((c) => 'error' in c).length} errors`);
}
