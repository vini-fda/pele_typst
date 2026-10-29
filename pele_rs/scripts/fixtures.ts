// Renders Mermaid sources with pele's JavaScript and writes what it gives, for tests/fixtures.rs
// to compare the Rust port against. Run with bun:
//
//   bun scripts/fixtures.ts <path to a pele checkout>
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const pele = process.argv[2];
if (!pele) throw new Error('usage: bun scripts/fixtures.ts <path to a pele checkout>');
const { render } = await import(join(pele, 'src/index.ts'));

const sources: string[] = [
  ...JSON.parse(readFileSync(join(pele, 'tests/corpus/flowchart-docs.json'), 'utf8')),
  ...JSON.parse(readFileSync(join(import.meta.dir, 'extra-flowcharts.json'), 'utf8')),
  ...JSON.parse(readFileSync(join(import.meta.dir, 'harvested-flowcharts.json'), 'utf8')),
];
const cases = sources.map((source) => {
  try {
    return { source, svg: render(source).svg };
  } catch (error) {
    return { source, error: (error as Error).message };
  }
});
writeFileSync(join(import.meta.dir, '../tests/fixtures/flowchart.json'), JSON.stringify(cases, null, 1));
console.log(`${cases.length} cases, ${cases.filter((c) => 'error' in c).length} errors`);
