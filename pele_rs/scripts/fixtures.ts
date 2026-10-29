// bun scripts/fixtures.ts <pinned Pele checkout> [--check]
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { execFileSync } from 'node:child_process';
import reference from './reference.json';

const pele = process.argv[2] && resolve(process.argv[2]);
if (!pele) throw new Error('usage: bun scripts/fixtures.ts <pinned Pele checkout> [--check]');
const check = process.argv.includes('--check');
process.env.TZ = reference.timeZone;
const version = JSON.parse(readFileSync(join(pele, 'package.json'), 'utf8')).version;
if (version !== reference.version) throw new Error(`expected Pele ${reference.version}, got ${version}`);
if (existsSync(join(pele, '.git'))) {
  const revision = execFileSync('git', ['-C', pele, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  if (revision !== reference.revision) throw new Error(`expected revision ${reference.revision}, got ${revision}`);
} else {
  const revision = readFileSync(join(pele, '.pele-reference-revision'), 'utf8').trim();
  if (revision !== reference.revision) throw new Error('unverified reference archive');
}
const { render, parse, detectType } = await import(join(pele, 'src/index.ts'));
const { metricsMeasurer } = await import(join(pele, 'src/text/measurer.ts'));
const { all } = await import(join(pele, 'src/diagrams/registry.ts'));
if (JSON.stringify(all.map((d: any) => d.type).sort()) !== JSON.stringify(Object.keys(reference.types).sort())) {
  throw new Error('reference manifest and upstream registry differ');
}
const literals = new Set<string>();
const literal = /`((?:[^`\\$]|\\.|\$(?!\{))*)`|'((?:[^'\\\n]|\\.)*)'|"((?:[^"\\\n]|\\.)*)"/g;
function walk(dir: string): void {
  for (const name of readdirSync(dir).sort()) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (/\.[jt]s$/.test(path)) {
      for (const m of readFileSync(path, 'utf8').matchAll(literal)) {
        try {
          const value = m[1] !== undefined ? eval('`' + m[1] + '`') : eval(m[2] !== undefined ? "'" + m[2] + "'" : '"' + m[3] + '"');
          if (typeof value === 'string' && value.includes('\n')) literals.add(value);
        } catch { /* Not a complete plain literal. */ }
      }
    }
  }
}
walk(join(pele, 'tests'));
const read = (path: string): any[] => existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : [];
const options = { now: reference.now };
function record(source: string, supplied: Record<string, unknown> = {}) {
  const opts = { ...options, ...supplied };
  const calls: unknown[] = [];
  const runtime: any = { ...opts, measurer: metricsMeasurer };
  if (opts.iconMode) runtime.icons = (name: string) => {
    calls.push(['icon', name]);
    return opts.iconMode === 'missing' ? null : opts.iconMode === 'empty' ? '' : '<path d="M2 2H22V22H2Z"/>';
  };
  if (opts.mathMode) runtime.math = (tex: string, size: number) => {
    calls.push(['math', tex, size]);
    if (opts.mathMode === 'missing') return null;
    const width = 30, height = opts.mathMode === 'invalid' ? -1 : opts.mathMode === 'tall' ? 60 : 18;
    return opts.mathMode === 'html' ? { html: '<span>x<br></span>', width, height } : { svg: '<path d="M1,1H9"/>', width, height };
  };
  let detected: string | null = null;
  try { detected = detectType(source); } catch { /* render records the structured error. */ }
  const errorData=(e:any)=>({message:e.message.toWellFormed(),code:e.code,type:e.type,mermaidType:e.mermaidType,line:e.line,column:e.column,snippet:e.snippet,
    ...(e.message.isWellFormed()?{}:{messageUtf16:Array.from({length:e.message.length},(_,i)=>e.message.charCodeAt(i))})});
  let model: any, parsed: any;
  try { model=parse(source,runtime); parsed={type:model.type,mermaidType:model.mermaidType}; }
  catch(e:any) { if(e.name!=='PeleError')throw e; parsed={error:errorData(e)}; }
  try {
    const result = render(source, runtime);
    const resolved = calls.slice();
    // Assert the reference itself supports reusable models under these options.
    // Rust repeats this sequence rather than storing four copies of every SVG.
    if (!model) throw new Error('render succeeded after parse failed');
    for (const maxWidth of [undefined, 320, 500, undefined]) {
      const next = { ...runtime, maxWidth };
      if (JSON.stringify(render(model, next)) !== JSON.stringify(render(source, next))) {
        throw new Error('reference model reuse changed the observable result');
      }
    }
    return { source, options: opts, detected, parsed, result, ...(opts.iconMode || opts.mathMode ? { calls: resolved } : {}), reuseWidths: [null, 320, 500, null] };
  } catch (e: any) {
    if (e.name !== 'PeleError') throw e;
    return { source, options: opts, detected, parsed, error:errorData(e) };
  }
}
let documentationCount = 0;
let agentflowReferenceCount = 0;
const coverage: Record<string, unknown> = {};
for (const [type, corpus] of Object.entries(reference.types)) {
  const docsPath = join(pele, `tests/corpus/${corpus}-docs.json`);
  if (!existsSync(docsPath)) throw new Error(`missing documentation corpus for ${type}`);
  const docs = read(docsPath);
  documentationCount += docs.length;
  const referenceCases = read(join(pele, `tests/corpus/${corpus}-reference.json`));
  if (type === 'agentflow') agentflowReferenceCount = referenceCases.length;
  const sources = new Set<string>([
    ...docs,
    ...referenceCases.map(c => typeof c === 'string' ? c : c.src),
    ...read(join(import.meta.dir, `extra/${type}.json`)),
  ]);
  for (const value of literals) {
    try { if (detectType(value) === type) sources.add(value); } catch { /* Invalid YAML lives in explicit corpora. */ }
  }
  const cases = [...sources].map(source => record(source));
  for (const source of docs) {
    cases.push(record(source, { maxWidth: 320 }));
    cases.push(record(source, { fontSize: 20, padding: 12, responsive: false, idPrefix: 'parity' }));
  }
  for (const source of sources) {
    if (source.includes('$$')) {
      for (const mathMode of ['svg', 'tall', 'html', 'missing', 'invalid']) cases.push(record(source, { mathMode, maxWidth: 320 }));
    }
    if (/fa[bklrs]?:|::icon|icon:/.test(source)) {
      for (const iconMode of ['all', 'missing', 'empty']) cases.push(record(source, { iconMode }));
    }
  }
  for (const supplied of [
    { monospace: [] }, { monospace: ['members', 'attributes', 'files', 'messages'] },
    { fontFamily: 'Custom Sans', fontFamilyMono: 'Custom Mono', fontSize: 11 },
    { responsive: true, maxWidth: 280, autoDirection: false, directionBreakpoint: 400 },
    { links: false, images: false }, { limit: 4 }, { outputLimit: 64 }, { maxEdges: 0 },
  ]) cases.push(record(docs[0], supplied));
  for (const item of read(join(import.meta.dir, `behavior/${type}.json`))) cases.push(record(item.source, item.options));
  for (const item of read(join(import.meta.dir, `regressions/${type}.json`))) cases.push(record(item.source, item.options));
  const path = join(import.meta.dir, `../tests/fixtures/${type}.json`);
  const output = JSON.stringify(cases, null, 1) + '\n';
  if (check) {
    if (!existsSync(path) || readFileSync(path, 'utf8') !== output) throw new Error(`${type}: fixtures are stale`);
  } else writeFileSync(path, output);
  console.log(`${type}: ${cases.length} cases (${cases.filter(c => 'error' in c).length} errors)`);
  coverage[type] = { corpus, documentation: docs.length, reference: referenceCases.length, cases: cases.length, errors: cases.filter(c => 'error' in c).length };
}
if (documentationCount !== 522 || agentflowReferenceCount !== 239) throw new Error('incomplete pinned reference corpus');
const summary = JSON.stringify({ revision: reference.revision, documentationCount, agentflowReferenceCount, types: coverage }, null, 2) + '\n';
const coveragePath = join(import.meta.dir, '../tests/fixtures/coverage.json');
if (check) { if (readFileSync(coveragePath, 'utf8') !== summary) throw new Error('coverage manifest is stale'); }
else writeFileSync(coveragePath, summary);
