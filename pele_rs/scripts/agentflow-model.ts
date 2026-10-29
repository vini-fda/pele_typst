// bun scripts/agentflow-model.ts <pinned Pele checkout> [--check]
// Public semantic models, source mappings, diagnostics, metadata, and lexer locations.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import reference from './reference.json';

const pele = process.argv[2] && resolve(process.argv[2]);
if (!pele) throw new Error('usage: bun scripts/agentflow-model.ts <pinned Pele checkout> [--check]');
const revision = existsSync(join(pele, '.git'))
  ? execFileSync('git', ['-C', pele, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
  : readFileSync(join(pele, '.pele-reference-revision'), 'utf8').trim();
if (revision !== reference.revision) throw new Error('wrong reference revision');
if (JSON.parse(readFileSync(join(pele, 'package.json'), 'utf8')).version !== reference.version) {
  throw new Error('wrong reference version');
}
const { parse } = await import(join(pele, 'src/index.ts'));
const { tokenize } = await import(join(pele, 'src/diagrams/agentflow/lexer.ts'));
const cases: any[] = [];
const sources = JSON.parse(readFileSync(join(import.meta.dir, '../tests/fixtures/agentflow.json'), 'utf8'));
for (const { source, options } of sources) {
  try {
    const db = parse(source, options);
    cases.push({
      source,
      options,
      model: {
        semantic: db.semanticModel(),
        diagnostics: db.diagnostics,
        mappings: db.mappings.map((m: any) => [
          m.id, m.type, m.position.startLine, m.position.startColumn,
          m.position.endLine, m.position.endColumn, m.position.startIndex, m.position.endIndex,
        ]),
        metadata: {
          vertices: [...db.nodes].map(([id, n]: any) => [id, n.metadata ?? null]),
          edges: db.edges.map((e: any) => [e.id, e.metadata ?? null]),
          flows: db.subgraphs.map((g: any) => [g.id, g.metadata ?? null]),
        },
      },
    });
  } catch (e: any) {
    cases.push({
      source, options,
      error: {
        message: e.message, code: e.code, type: e.type, mermaidType: e.mermaidType,
        line: e.line, column: e.column, snippet: e.snippet,
      },
    });
  }
}
const raw: string[] = [];
for (const newline of ['\n', '\r', '\r\n', '\u2028', '\u2029']) {
  for (const body of [
    `a[one${newline}two]`,
    `a(-one${newline}two-)`,
    `a --one${newline}two--> b`,
    `a@{label:"one${newline}two"}`,
    `a[one%%comment${newline}two]`,
    `%% comment${newline}a --> b`,
  ]) {
    raw.push(`agentflow-beta TB${newline}${body}`);
  }
}
const tokens = raw.map(source => ({ source, tokens: tokenize(source) }));
const output = JSON.stringify({ cases, tokens }, null, 1) + '\n';
const path = join(import.meta.dir, '../tests/fixtures/agentflow-model.json');
if (process.argv.includes('--check')) {
  if (readFileSync(path, 'utf8') !== output) throw new Error('Agentflow model fixtures are stale');
} else {
  writeFileSync(path, output);
}
console.log(`${cases.length} agentflow models/errors and ${tokens.length} source-location token streams`);
