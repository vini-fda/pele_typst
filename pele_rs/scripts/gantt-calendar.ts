// bun scripts/gantt-calendar.ts <pinned Pele checkout> [--check]
// Calendar and model oracle, including DST and the full ECMAScript Date range.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join, resolve } from 'node:path';
import reference from './reference.json';

const pele = process.argv[2] && resolve(process.argv[2]);
if (!pele) throw new Error('usage: bun scripts/gantt-calendar.ts <pinned Pele checkout> [--check]');
if (JSON.parse(readFileSync(join(pele, 'package.json'), 'utf8')).version !== reference.version) {
  throw new Error('wrong reference version');
}
const revision = existsSync(join(pele, '.git'))
  ? execFileSync('git', ['-C', pele, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
  : readFileSync(join(pele, '.pele-reference-revision'), 'utf8').trim();
if (revision !== reference.revision) throw new Error('wrong reference revision');
const dates = await import(join(pele, 'src/diagrams/gantt/dates.ts'));
const axis = await import(join(pele, 'src/diagrams/gantt/axis.ts'));
const { GanttDb } = await import(join(pele, 'src/diagrams/gantt/db.ts'));
const { parseGantt } = await import(join(pele, 'src/diagrams/gantt/parser.ts'));
const { render } = await import(join(pele, 'src/index.ts'));
const { metricsMeasurer } = await import(join(pele, 'src/text/measurer.ts'));
const cases: any[] = [];
for (const zone of ['UTC', 'America/Los_Angeles']) {
  process.env.TZ = zone;
  const now = +new Date(2031, 4, 6, 9, 30, 15, 250);
  const record = (op: string, args: unknown[], value: unknown) => {
    cases.push({ zone, now, op, args, value: value === undefined ? null : value });
  };
  const formats = [
    'YYYY-MM-DD', 'DD-MM-YYYY', 'YY/M/D', 'YYYY-MM-DD HH:mm', 'YYYY-MM-DD H:m:s',
    'YYYY-MM-DDTHH:mm:ss.SSS', 'h:mm A', 'hh:mm a', 'MMM D, YYYY', 'Do MMMM YYYY',
    '[on] YYYY-MM-DD', 'HH:mm', 'YYYY', 'MM', 'D', 'ss', 'Q YYYY', 'dddd YYYY',
    'W WW GGGG', 'GGGG-[W]WW', 'YYYY-MM-DDTHH:mm:ssZ', 'YYYY-MM-DDTHH:mm:ssZZ',
    'x', 'X', 'Y YYY', 'S SS', '[w] YYYY', 'w', 'zzz',
  ];
  const axes = [
    '%Y-%m-%d', '%a %A %b %B', '%H:%M:%S.%L', '%I %p', '%e|%-d|%_d|%0e',
    '%j', '%U %W %V', '%u %w', '%y %G %g', '%q', '%x', '%X', '%c', '%f',
    '%%Y %k %', '%s', '%Q', '%Z',
  ];
  const samples = [
    new Date(2024, 1, 3, 15, 4, 5, 67), new Date(2021, 0, 3), new Date(2024, 11, 30),
    new Date(2024, 2, 10, 1, 30), new Date(2024, 2, 10, 3, 30),
    new Date(2024, 10, 3, 1, 30), new Date(2024, 10, 3, 2, 30),
    new Date(0), new Date(-1), new Date(8.64e15), new Date(-8.64e15),
  ];
  for (let i = 0; i < 30; i++) {
    samples.push(new Date(1971 + i * 2, (i * 7) % 12, 1 + (i * 13) % 28,
      (i * 11) % 24, (i * 17) % 60, (i * 19) % 60, i * 31 % 1000));
  }
  for (const date of samples) {
    for (const f of formats) {
      const value = dates.format(date, f);
      record('format', [+date, f], value);
      if (value !== undefined) record('parse', [value, f], dates.parseStrict(value, f, new Date(now))?.getTime());
    }
    for (const f of axes) record('axis', [+date, f], axis.timeFormat(f, date));
    for (const [value, unit] of [
      [1, 'd'], [2, 'd'], [0.5, 'w'], [1.5, 'M'], [1, 'y'], [24, 'h'], [1, 'ms'], [99999999, 'y'],
    ] as const) {
      record('add', [+date, value, unit], +dates.add(date, value, unit));
    }
  }
  for (const [input, template] of [
    ['2014-1-6', 'YYYY-MM-DD'], ['2014-13-01', 'YYYY-MM-DD'], ['2014-02-30', 'YYYY-MM-DD'],
    ['2024-03-10 02:30', 'YYYY-MM-DD HH:mm'], ['2024-11-03 01:30', 'YYYY-MM-DD HH:mm'],
    ['2014-01-06 ', 'YYYY-MM-DD'], ['25:00', 'HH:mm'], ['5 January 2024', 'Do MMMM YYYY'],
    ['Tuesday 2024', 'dddd YYYY'], ['Invalid Date', ''], ['12/25/2020', 'L'],
    ['2020-01', 'YYYY-ww'], ['1410715640.579', 'X'], ['1e3', 'x'], ['-5', 'x'],
  ]) {
    record('parse', [input, template], dates.parseStrict(input, template, new Date(now))?.getTime());
  }
  for (const [start, stop] of [
    [+new Date(2024, 2, 9), +new Date(2024, 2, 12)],
    [+new Date(2024, 10, 2), +new Date(2024, 10, 5)],
    [+new Date(2024, 0, 1), +new Date(2024, 3, 2)],
    [0, 71000], [0, 30], [-8.64e15, -8.64e15 + 120000], [8.64e15 - 120000, 8.64e15],
  ]) {
    for (const count of [2, 4, 10]) record('auto', [start, stop, count], axis.autoTicks(start, stop, count));
    for (const interval of [
      '1millisecond', '2second', '15minute', '6hour', '1day', '10day', '2week', '1month', '99999999second', '1year',
    ]) {
      for (const weekday of ['sunday', 'monday']) {
        record('interval', [interval, weekday, start, stop, 100], axis.intervalTicks(interval, weekday, start, stop, 100));
      }
    }
  }
  for (const source of [
    'gantt\nTask :2d',
    'gantt\ndateFormat YYYY-MM-DD\nA :a, 2024-03-09, 2d\nB :after a, 24h',
    'gantt\ndateFormat YYYY-MM-DD\nA :2024-11-02, 2d',
    'gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nincludes 2024-03-10\nA :a, 2024-03-08, 3d\nB :b, 2024-03-08, until a\nC :after b, 1d',
    'gantt\ndateFormat YYYY-MM-DD\nA :a, after b, 1d\nB :b, after c, 2d\nC :c, 2024-03-09, 1d',
    'gantt\ndateFormat x\nA :8640000000000000, 1ms',
    'gantt\ndateFormat YYYY-MM-DD\nA :0001-01-01, 1d\nB :2024-01-01, 99990000d',
  ]) {
    const db = new GanttDb();
    parseGantt(source, db);
    for (const at of [now, now + 86400000]) {
      const tasks = db.getTasks(at).map((t: any) => ({
        id: t.id,
        start: t.startTime === undefined ? null : +t.startTime,
        end: t.endTime === undefined ? null : +t.endTime,
        renderEnd: t.renderEndTime === null ? null : +t.renderEndTime,
        manual: t.manualEndTime,
        processed: t.processed,
        order: t.order,
      }));
      record('schedule', [source, at], tasks);
    }
    record('render', [source], render(source, { now, measurer: metricsMeasurer }));
  }
}
const path = join(import.meta.dir, '../tests/fixtures/gantt-calendar.json');
const output = JSON.stringify(cases, null, 1) + '\n';
if (process.argv.includes('--check')) {
  if (readFileSync(path, 'utf8') !== output) throw new Error('Gantt calendar fixtures are stale');
} else {
  writeFileSync(path, output);
}
console.log(`${cases.length} Gantt calendar cases`);
