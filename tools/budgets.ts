// Budgets fail CI (ADR-0011, plan §8). The UI half, measured on the build (`npm run build --workspace
// @wisp/web` first): what the first screen loads, brotli-compressed at the quality the edge serves
// (crates/wisp-api statics: brotli 11):
//   first screen JS / CSS   every file each look's first screen fetches   ≤ its ratchet (FIRST_SCREEN_BUDGETS), the goal 120 / 30 KB br
//   initial JS / CSS        index.html's own files                         ≤ 120 / 30 KB br
// "The first screen" (ADR-0105): the built app opened cold in headless Chrome, at a desktop width and as the phone app,
// up to the shell's `wisp:first-screen` mark (drawn, the home list here, painted): tools/first-screen.ts. The tags
// index.html names (the entry and its preloads) missed every lazy chunk the first screen asks for (perf audit r1 #1:
// 118 KB counted, 309 KB loaded): they stay gated at 120 / 30, and each look's first screen is gated at its own
// ratchet, FIRST_SCREEN_BUDGETS (ADR-0105 §4), which only goes down toward 120 / 30. Lazy chunks that load after the
// mark (a task's view, the palette, the plugins' later parts) are listed, not budgeted. CI
// (.github/workflows/suite.yml) runs this after the web build and fails on a miss; under GitHub Actions the numbers
// also go to the job summary ($GITHUB_STEP_SUMMARY). `--entry-only` counts index.html's tags alone (no browser: the
// tool's own tests). The server half is measured by the Rust spikes and gated there: `SERVER_BUDGETS` by
// spikes/server-budgets (a CI step, ADR-0080, ADR-0082: cold start, idle RSS, the list, patch latency, append rate,
// write amplification), `EVENT_BUDGETS` by spikes/events (not a CI step; its append rate is the function
// spikes/server-budgets gates).
//
//   node tools/budgets.ts [--dist=apps/web/dist] [--json] [--summary=<file>] [--entry-only]

import { brotliCompressSync, constants } from 'node:zlib';
import { appendFileSync, readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, relative } from 'node:path';
import { firstScreen, LOOKS, type Look, type Loaded } from './first-screen.ts';

/** ADR-0011's initial budgets: the goal for each look's first screen, and the gate on index.html's own files. */
export const BUDGETS = { initialJsBr: 120 * 1024, initialCssBr: 30 * 1024 } as const;

export interface LookBudget {
  readonly jsBr: number;
  readonly cssBr: number;
}

/** Each look's first screen, gated (ADR-0105 §4): a ratchet set at the honest numbers plus 2 KB on 2026-11-19
 * (desktop JS 171.1, phone JS 164.9, desktop CSS 28.6, phone CSS 39.5 KB br). It only goes down: a cut that lands
 * lowers it to the new number plus 2 KB, toward BUDGETS; raising it needs a new ADR. */
export const FIRST_SCREEN_BUDGETS: Readonly<Record<Look, LookBudget>> = {
  desktop: { jsBr: 173 * 1024, cssBr: 30 * 1024 },
  phone: { jsBr: 167 * 1024, cssBr: 40.5 * 1024 },
};

/** `wisp serve`'s budgets (plan §8) on a home of 5 000 tasks, measured and gated in PR CI by `spikes/server-budgets` (it
 * exits 1 on a miss; its constants must equal these: tools/test/budgets.test.ts). */
export const SERVER_BUDGETS = {
  /** process start → the first successful HTTP response: the median of 5 rounds, each the median of 5 restarts (ADR-0089) */
  coldStartMs: 150,
  /** the server's resident set once settled, after a client read the list */
  idleRssMb: 120,
  /** the UI's first read (`GET /api/events?fresh=list` → the list snapshot's first page, ADR-0081), the median of 5
   * rounds of 12 reads (ADR-0089) */
  listMs: 5,
  /** a running task's output written → each client's patch carrying it, p99; 10 tasks streaming, 4 clients (ADR-0082);
   * the median of 3 rounds' p99 (ADR-0089) */
  patchP99Ms: 150,
  /** durable events a second into the event log, 1 000 a transaction (spikes/events' `append_rate`) */
  appendPerS: 20_000,
  /** bytes the store wrote (WAL + database growth) ÷ the activity the 10 streaming tasks kept, at real agents' step
   * sizes and rates (ADR-0085: a ratchet, lowered by each cut that lands; ADR-0088: 48 → 44, other plugins' per-turn
   * commits cut, 35.9–39.4 measured on macOS; Linux reads ~1.5 above) */
  writeAmplification: 44,
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
  /** write amplification of a bulk append (one session streaming 2 MB in 5 000 events: the runtime bench, ADR-0085) */
  writeAmplification: 15,
} as const;

const arg = (name: string): string | undefined => process.argv.find((a) => a.startsWith(`--${name}=`))?.slice(name.length + 3);
const dist = arg('dist') ?? join(import.meta.dirname, '..', 'apps', 'web', 'dist');

const br = (buf: Buffer): number => brotliCompressSync(buf, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_SIZE_HINT]: buf.length } }).length;
const kb = (n: number): string => `${(n / 1024).toFixed(1)} KB`;

export interface Measured {
  /** index.html's own files (its module script, modulepreloads and stylesheets): gated at BUDGETS */
  entry: { js: { files: string[]; raw: number; br: number }; css: { files: string[]; raw: number; br: number } };
  /** the largest look's, each on its own (JS and CSS may come from different looks; listed in --json, not gated: the
   *  gate reads `entry` and `looks`); index.html's own files when no look was measured */
  initialJs: { files: string[]; raw: number; br: number };
  initialCss: { files: string[]; raw: number; br: number };
  /** each look's own, when the first screen was measured in a browser */
  looks?: Partial<Record<Look, { js: { files: string[]; raw: number; br: number }; css: { files: string[]; raw: number; br: number } }>>;
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

/** `loaded`: what each look's first screen fetched (tools/first-screen.ts); without it, index.html's tags alone. */
export function measure(dir: string, loaded?: Partial<Record<Look, Loaded>>): Measured {
  const html = readFileSync(join(dir, 'index.html'), 'utf8');
  const tags = initialFiles(html);
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
  const looks: Measured['looks'] = {};
  for (const look of LOOKS) {
    const l = loaded?.[look];
    if (l) looks[look] = { js: sum([...l.js]), css: sum([...l.css]) };
  }
  const worst = (k: 'js' | 'css'): { files: string[]; raw: number; br: number } | undefined =>
    Object.values(looks).reduce<{ files: string[]; raw: number; br: number } | undefined>((w, x) => (x && (!w || x[k].br > w.br) ? x[k] : w), undefined);
  // what any look loaded first is not lazy
  const initial = new Set([...tags.js, ...tags.css, ...Object.values(looks).flatMap((x) => (x ? [...x.js.files, ...x.css.files] : []))].map((u) => u.replace(/^\//, '')));
  const assets = existsSync(join(dir, 'assets')) ? readdirSync(join(dir, 'assets')).map((f) => `assets/${f}`) : [];
  const later = (ext: string): { file: string; br: number }[] =>
    assets
      .filter((f) => f.endsWith(ext) && !initial.has(f))
      .map((file) => ({ file, br: br(readFileSync(join(dir, file))) }))
      .sort((a, b) => b.br - a.br);
  const entry = { js: sum(tags.js), css: sum(tags.css) };
  return { entry, initialJs: worst('js') ?? entry.js, initialCss: worst('css') ?? entry.css, ...(loaded && { looks }), lazy: later('.js'), lazyCss: later('.css') };
}

/** Each budget the build misses, in words: index.html's own files against `budgets`, each measured look's first screen
 *  against its ratchet. */
export function overBudget(m: Measured, budgets: { initialJsBr: number; initialCssBr: number } = BUDGETS, firstScreen: Readonly<Record<Look, LookBudget>> = FIRST_SCREEN_BUDGETS): string[] {
  const over: string[] = [];
  if (m.entry.js.br > budgets.initialJsBr) over.push(`initial JS ${kb(m.entry.js.br)} br > ${kb(budgets.initialJsBr)}`);
  if (m.entry.css.br > budgets.initialCssBr) over.push(`initial CSS ${kb(m.entry.css.br)} br > ${kb(budgets.initialCssBr)}`);
  for (const look of LOOKS) {
    const x = m.looks?.[look];
    if (!x) continue;
    const b = firstScreen[look];
    if (x.js.br > b.jsBr) over.push(`first screen JS (${look}) ${kb(x.js.br)} br > ${kb(b.jsBr)}`);
    if (x.css.br > b.cssBr) over.push(`first screen CSS (${look}) ${kb(x.css.br)} br > ${kb(b.cssBr)}`);
  }
  return over;
}

/** The job summary: every gated number against its budget, then the largest lazy chunks (markdown). */
export function summary(m: Measured, budgets: { initialJsBr: number; initialCssBr: number } = BUDGETS, firstScreen: Readonly<Record<Look, LookBudget>> = FIRST_SCREEN_BUDGETS, top = 10): string {
  const row = (name: string, got: { br: number; raw: number; files: string[] }, max: number): string =>
    `| ${name} | ${kb(got.br)} | ${kb(max)} | ${kb(got.raw)} | ${got.files.length} | ${got.br > max ? `❌ over by ${kb(got.br - max)}` : '✅'} |`;
  const lazy = (list: { file: string; br: number }[]): string =>
    list.length === 0 ? 'none' : list.slice(0, top).map((l) => `\`${l.file.replace('assets/', '')}\` ${kb(l.br)}`).join(', ');
  return [
    '### Bundle budgets (ADR-0011, ADR-0105)',
    '',
    '| | brotli | budget | raw | files | |',
    '|---|---:|---:|---:|---:|---|',
    ...LOOKS.flatMap((look) => {
      const x = m.looks?.[look];
      return x ? [row(`first screen JS (${look})`, x.js, firstScreen[look].jsBr), row(`first screen CSS (${look})`, x.css, firstScreen[look].cssBr)] : [];
    }),
    row('initial JS', m.entry.js, budgets.initialJsBr),
    row('initial CSS', m.entry.css, budgets.initialCssBr),
    '',
    ...(m.looks ? [`First screen: a ratchet toward ${kb(budgets.initialJsBr)} JS / ${kb(budgets.initialCssBr)} CSS (ADR-0105 §4). Initial: index.html's own files.`, ''] : []),
    `Lazy JS: ${m.lazy.length} chunks, largest ${lazy(m.lazy)}.`,
    '',
    `Lazy CSS: ${m.lazyCss.length} sheets, largest ${lazy(m.lazyCss)}.`,
    '',
  ].join('\n');
}

async function main(): Promise<void> {
  if (!existsSync(join(dist, 'index.html'))) {
    console.error(`budgets: no build at ${relative(process.cwd(), dist)} (npm run build --workspace @wisp/web)`);
    process.exitCode = 1;
    return;
  }
  let loaded: Awaited<ReturnType<typeof firstScreen>> | undefined;
  if (!process.argv.includes('--entry-only'))
    try {
      loaded = await firstScreen(dist);
    } catch (err) {
      // no Chrome, a look that never marked: a failure the job shows, never a silent pass
      const why = `the first screen wasn't measured: ${err instanceof Error ? err.message : String(err)}`;
      const to = arg('summary') ?? process.env['GITHUB_STEP_SUMMARY'];
      if (to) appendFileSync(to, `### Bundle budgets (ADR-0011, ADR-0105)\n\n❌ ${why}\n`);
      console.error(`${process.env['GITHUB_ACTIONS'] ? '::error title=Bundle budget::' : ''}${why}`);
      process.exitCode = 1;
      return;
    }
  const m = measure(dist, loaded);
  const over = overBudget(m);
  const to = arg('summary') ?? process.env['GITHUB_STEP_SUMMARY'];
  if (to) appendFileSync(to, summary(m));
  if (process.argv.includes('--json')) {
    console.log(JSON.stringify({ ...m, budgets: BUDGETS, firstScreenBudgets: FIRST_SCREEN_BUDGETS, over }, null, 2));
  } else {
    const line = (name: string, x: { br: number; raw: number; files: string[] }, max: number): void =>
      console.log(`${name.padEnd(28)} ${kb(x.br).padStart(9)} br  (${kb(x.raw)} raw, ${x.files.length} files)  budget ${kb(max)}`);
    for (const look of LOOKS) {
      const x = m.looks?.[look];
      if (!x) continue;
      line(`first screen JS (${look})`, x.js, FIRST_SCREEN_BUDGETS[look].jsBr);
      line(`first screen CSS (${look})`, x.css, FIRST_SCREEN_BUDGETS[look].cssBr);
    }
    line('initial JS', m.entry.js, BUDGETS.initialJsBr);
    line('initial CSS', m.entry.css, BUDGETS.initialCssBr);
    console.log(`lazy chunks  ${m.lazy.length}, largest: ${m.lazy.slice(0, 5).map((l) => `${l.file.replace('assets/', '')} ${kb(l.br)}`).join(', ') || 'none'}`);
    console.log(`lazy CSS     ${m.lazyCss.length}, largest: ${m.lazyCss.slice(0, 5).map((l) => `${l.file.replace('assets/', '')} ${kb(l.br)}`).join(', ') || 'none'}`);
    // a GitHub annotation on the run as well as the log line
    for (const o of over) console.error(`${process.env['GITHUB_ACTIONS'] ? '::error title=Bundle budget::' : ''}OVER BUDGET: ${o}`);
  }
  process.exitCode = over.length > 0 ? 1 : 0;
}

if (import.meta.filename === process.argv[1]) await main();
