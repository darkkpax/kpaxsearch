import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';
import { search } from '../src/search.ts';

const path = process.argv[2];
if (!path) throw new Error('usage: node scripts/benchmark.mjs path/to/programs.json');
const programs = JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''));
const queries = ['code', 'da', 'chrome', 'вшы', 'vsc', 'rfkmr', 'visual studio', 'xyzxyz'];
for (let i = 0; i < 100; i++) search(programs, queries[i % queries.length]);
const times = [];
for (let i = 0; i < 1000; i++) {
  const start = performance.now();
  search(programs, queries[i % queries.length]);
  times.push(performance.now() - start);
}
times.sort((a, b) => a - b);
console.log(JSON.stringify({ programs: programs.length, samples: times.length, medianMs: +times[500].toFixed(3), p95Ms: +times[950].toFixed(3), maxMs: +times[999].toFixed(3), scope: 'ranking only, excluding rendering and app startup' }, null, 2));
