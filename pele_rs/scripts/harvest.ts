// Collects flowchart sources written as string literals in pele's tests, for a wider corpus.
//   bun scripts/harvest.ts <path to a pele checkout> > scripts/harvested-flowcharts.json
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = join(process.argv[2], 'tests');
const files: string[] = [];
const walk = (dir: string) => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (path.endsWith('.ts')) files.push(path);
  }
};
walk(root);
const found = new Set<string>();
const literal = /`((?:[^`\\$]|\\.|\$(?!\{))*)`|'((?:[^'\\\n]|\\.)*)'|"((?:[^"\\\n]|\\.)*)"/g;
for (const file of files) {
  const text = readFileSync(file, 'utf8');
  for (const m of text.matchAll(literal)) {
    const raw = m[1] ?? m[2] ?? m[3];
    let value: string;
    try {
      value = m[1] !== undefined ? eval('`' + raw + '`') : JSON.parse('"' + raw.replace(/\\'/g, "'").replace(/"/g, '\\"').replace(/\\\\"/g, '\\"') + '"');
    } catch {
      continue;
    }
    if (/^\s*(flowchart|graph)\b/.test(value) && value.includes('\n')) found.add(value);
  }
}
console.log(JSON.stringify([...found], null, 1));
