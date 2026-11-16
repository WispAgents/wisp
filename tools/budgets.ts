// Budgets fail CI (ADR-0011, plan §8). The UI half, measured on the build (`npm run build --workspace
// @wisp/web` first): what the first page load needs, brotli-compressed at the quality the edge
// serves (crates/wisp-api statics: brotli 11):
//   initial JS   the entry module and every modulepreload index.html names   ≤ 120 KB br
//   initial CSS  every stylesheet index.html links                              ≤ 30 KB br
// Lazy chunks (a task's view, the palette, a plugin's screens) and the stylesheets that load with them are
// listed, not budgeted: they load when they show. CI (.github/workflows/suite.yml) runs this after the web build and
// fails on a miss; under GitHub Actions the numbers also go to the job summary ($GITHUB_STEP_SUMMARY). The server half
// is measured by the Rust spikes and gated there: `SERVER_BUDGETS` by spikes/server-budgets (a CI step, ADR-0080,
// ADR-0082: cold start, idle RSS, the list, patch latency, append rate, write amplification), `EVENT_BUDGETS` by
// spikes/events (not a CI step; its append rate is the function spikes/server-budgets gates).
//
//   node tools/budgets.ts [--dist=apps/web/dist] [--json] [--summary=<file>]

import { brotliCompressSync, constants } from 'node:zlib';
import { appendFileSync, readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';

export const BUDGETS = { initialJsBr: 120 * 1024, initialCssBr: 30 * 1024 } as const;

/** `wisp serve`'s budgets (plan §8) on a home of 5 000 tasks, measured and gated in PR CI by `spikes/server-budgets` (it
 * exits 1 on a miss; its constants must equal these: tools/test/budgets.test.ts). */
export const SERVER_BUDGETS = {
  /** process start → the first successful HTTP response, median of 7 restarts */
  coldStartMs: 150,
  /** the server's resident set once settled, after a client read the list */
  idleRssMb: 120,
  /** the UI's first read (`GET /api/events?fresh=list` → the list snapshot's first page, ADR-0081), median of 30 */
  listMs: 5,
  /** a running task's output written → each client's patch carrying it, p99; 10 tasks streaming, 4 clients (ADR-0082) */
  patchP99Ms: 150,
  /** durable events a second into the event log, 1 000 a transaction (spikes/events' `append_rate`) */
  appendPerS: 20_000,
  /** bytes the store wrote (WAL + database growth) ÷ the activity the 10 streaming tasks kept */
  writeAmplification: 15,
} as const;

/** The durable event log's budgets (ADR-0044), measured and gated by `spikes/events` (it exits 1 on a miss). */
export const EVENT_BUDGETS = {
  /** emit → in-process handler, p99 */
  emitToHandlerP99Ms: 1,
  /** durable appends a second: the server's budget, gated in PR CI by spikes/server-budgets */
  durableAppendPerS: SERVER_BUDGETS.appendPerS,
  /** replay of 100 000 events to a durable subscription */
  replay100kMs: 1_000,
  /** the events:* topic to a client, p99 (the sync path's budget: spikes/sync-bench-rs) */
  topicP99Ms: 150,
  /** write amplification on the streaming bench: unchanged, since stream events stay ephemeral (the server's budget) */
  writeAmplification: SERVER_BUDGETS.writeAmplification,
} as const;

const arg = (name: string): string | undefined => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3);
const dist = arg('dist') ?? join(import.meta.dirname, '..', 'apps', 'web', 'dist');

const br = (buf: Buffer): number => brotliCompressSync(buf, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_SIZE_HINT]: buf.length } }).length;
const kb = (n: number): string => `${(n / 1024).toFixed(1)} KB`;

export interface Measured {
  initialJs: { files: string[]; raw: number; br: number };
  initialCss: { files: string[]; raw: number; br: number };
  lazy: { file: string; br: number }[];
  lazyCss: { file: string; br: number }[];
}

/** The files the first load fetches, read from index.html's tags. */
export function initialFiles(html: string): { js: string[]; css: string[] } {
  const js = new Set<string>();
  const css = new Set<string>();
  for (const m of html.matchAll(/<script\b[^>]*type="module"[^>]*\bsrc="([^"]+)"/g)) if (m[1]) js.add(m[1]);
  for (const m of html.matchAll(/<link\b[^>]*rel="modulepreload"[^>]*\bhref="([^"]+)"/g)) if (m[1]) js.add(m[1]);
  for (const m of html.matchAll(/<link\b[^>]*rel="stylesheet"[^>]*\bhref="([^"]+)"/g)) if (m[1]) css.add(m[1]);
  return { js: [...js], css: [...css] };
}

export function measure(dir: string): Measured {
  const html = readFileSync(join(dir, 'index.html'), 'utf8');
  const { js, css } = initialFiles(html);
  const read = (url: string): Buffer => readFileSync(join(dir, url.replace(/^\//, '')));
  const sum = (urls: string[]): { files: string[]; raw: number; br: number } => {
    let raw = 0;
    let b = 0;
    for (const u of urls) {
      const buf = read(u);
      raw += buf.length;
      b += br(buf);
    }
    return { files: urls, raw, br: b };
  };
  const initial = new Set([...js, ...css].map((u) => u.replace(/^\//, '')));
  const assets = existsSync(join(dir, 'assets')) ? readdirSync(join(dir, 'assets')).map((f) => `assets/${f}`) : [];
  const later = (ext: string): { file: string; br: number }[] =>
    assets
      .filter((f) => f.endsWith(ext) && !initial.has(f))
      .map((file) => ({ file, br: br(readFileSync(join(dir, file))) }))
      .sort((a, b) => b.br - a.br);
  return { initialJs: sum(js), initialCss: sum(css), lazy: later('.js'), lazyCss: later('.css') };
}

/** Each budget the build misses, in words. */
export function overBudget(m: Measured, budgets: { initialJsBr: number; initialCssBr: number } = BUDGETS): string[] {
  const over: string[] = [];
  if (m.initialJs.br > budgets.initialJsBr) over.push(`initial JS ${kb(m.initialJs.br)} br > ${kb(budgets.initialJsBr)}`);
  if (m.initialCss.br > budgets.initialCssBr) over.push(`initial CSS ${kb(m.initialCss.br)} br > ${kb(budgets.initialCssBr)}`);
  return over;
}

/** The job summary: both budgets with their numbers, then the largest lazy chunks (markdown). */
export function summary(m: Measured, budgets: { initialJsBr: number; initialCssBr: number } = BUDGETS, top = 10): string {
  const row = (name: string, got: { br: number; raw: number; files: string[] }, max: number): string =>
    `| ${name} | ${kb(got.br)} | ${kb(max)} | ${kb(got.raw)} | ${got.files.length} | ${got.br > max ? `❌ over by ${kb(got.br - max)}` : '✅'} |`;
  const lazy = (list: { file: string; br: number }[]): string =>
    list.length === 0 ? 'none' : list.slice(0, top).map((l) => `\`${l.file.replace('assets/', '')}\` ${kb(l.br)}`).join(', ');
  return [
    '### Bundle budgets (ADR-0011)',
    '',
    '| | brotli | budget | raw | files | |',
    '|---|---:|---:|---:|---:|---|',
    row('initial JS', m.initialJs, budgets.initialJsBr),
    row('initial CSS', m.initialCss, budgets.initialCssBr),
    '',
    `Lazy JS: ${m.lazy.length} chunks, largest ${lazy(m.lazy)}.`,
    '',
    `Lazy CSS: ${m.lazyCss.length} sheets, largest ${lazy(m.lazyCss)}.`,
    '',
  ].join('\n');
}

function main(): void {
  if (!existsSync(join(dist, 'index.html'))) {
    console.error(`budgets: no build at ${relative(process.cwd(), dist)} (npm run build --workspace @wisp/web)`);
    process.exitCode = 1;
    return;
  }
  const m = measure(dist);
  const over = overBudget(m);
  const to = arg('summary') ?? process.env['GITHUB_STEP_SUMMARY'];
  if (to) appendFileSync(to, summary(m));
  if (process.argv.includes('--json')) {
    console.log(JSON.stringify({ ...m, budgets: BUDGETS, over }, null, 2));
  } else {
    console.log(`initial JS   ${kb(m.initialJs.br).padStart(9)} br  (${kb(m.initialJs.raw)} raw, ${m.initialJs.files.length} files)  budget ${kb(BUDGETS.initialJsBr)}`);
    console.log(`initial CSS  ${kb(m.initialCss.br).padStart(9)} br  (${kb(m.initialCss.raw)} raw, ${m.initialCss.files.length} files)  budget ${kb(BUDGETS.initialCssBr)}`);
    console.log(`lazy chunks  ${m.lazy.length}, largest: ${m.lazy.slice(0, 5).map((l) => `${l.file.replace('assets/', '')} ${kb(l.br)}`).join(', ') || 'none'}`);
    console.log(`lazy CSS     ${m.lazyCss.length}, largest: ${m.lazyCss.slice(0, 5).map((l) => `${l.file.replace('assets/', '')} ${kb(l.br)}`).join(', ') || 'none'}`);
    // a GitHub annotation on the run as well as the log line
    for (const o of over) console.error(`${process.env['GITHUB_ACTIONS'] ? '::error title=Bundle budget::' : ''}OVER BUDGET: ${o}`);
  }
  process.exitCode = over.length > 0 ? 1 : 0;
}

if (import.meta.filename === process.argv[1]) main();
