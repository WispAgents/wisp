// The kit's Changes list (app/Changes.tsx): v1 features/changes.js's stage, revert and commit controls.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { createSignal, type JSX } from 'solid-js';
import { ChangesList, firstLine, hunkKey, hunkRuns, type ChangeFile, type ChangesLabels, type ChangeRow } from '../src/app/Changes.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

function mount(ui: () => JSX.Element): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(ui, host);
  return host;
}

const L: ChangesLabels = {
  files: (n) => `${n} files`,
  vs: 'vs main',
  vsTitle: 'compared with main',
  expandAll: 'Expand all',
  collapseAll: 'Collapse all',
  stage: 'Stage',
  part: 'Partly',
  staged: 'Staged',
  viewed: 'Viewed',
  viewedTitle: (on) => (on ? 'Viewed' : 'Mark viewed'),
  edit: 'Edit',
  more: 'More',
  line: (n) => `Line ${n}`,
  revert: 'Revert',
  revertTitle: 'Put this hunk back',
  stageHunk: 'Stage',
  unstageHunk: 'Unstage',
  unchanged: (n) => `${n} unchanged`,
  commit: 'Commit',
  commitPlaceholder: 'Fix the sum',
};

const ROWS: ChangeRow[] = [
  { op: ' ', text: 'export function add(a, b) {', a: 1, b: 1 },
  { op: '-', text: '  return a - b;', a: 2 },
  { op: '+', text: '  return a + b;', b: 2 },
  { op: '…', count: 40 },
  { op: '-', text: 'old', a: 43 },
  { op: ' ', text: '}', a: 44, b: 43 },
];

test('a hunk key is v1 hash(): djb2 per code point in base 36, the same key wisp-git checks', () => {
  // the vectors of plugins/git/core/src/hunks.rs (hunk_key_is_v1s_hash)
  expect(hunkKey([])).toBe('45h');
  expect(hunkKey([{ op: '+', text: 'a', b: 1 }])).toBe('375t97');
  expect(hunkKey([{ op: '…', count: 3 }])).toBe('3cgvfs');
  expect(hunkKey([{ op: ' ', text: 'é😀', a: 1, b: 1 }])).toBe('yk5m0l');
});

test('a diff splits into hunks at each gap, each knowing the gap before it and its first changed line', () => {
  const hs = hunkRuns(ROWS);
  expect(hs.map((h) => [h.from, h.rows.length, h.gap?.count ?? null])).toEqual([
    [0, 3, null],
    [4, 2, 40],
  ]);
  expect(hs.map((h) => firstLine(h.rows))).toEqual([2, 43]); // a line only removed reads its old number
  expect(hunkKey(hs[0]?.rows ?? [])).not.toBe(hunkKey(hs[1]?.rows ?? []));
});

/** a file outside a worktree: no staging */
const UNSTAGED: ChangeFile = { path: 'src/math.js', status: 'M', add: 1, del: 2, binary: false, rows: ROWS, revertable: true };
const FILE: ChangeFile = { ...UNSTAGED, stage: 'part', hunkStage: ['unstage', 'stage'] };

test('stage, stage a hunk, revert a hunk and commit call back with the path, hunk and key', () => {
  const calls: string[] = [];
  const host = mount(() => (
    <ChangesList
      files={[FILE]}
      labels={L}
      open={() => true}
      viewed={() => false}
      onToggle={() => {}}
      onViewed={(p) => calls.push(`viewed ${p}`)}
      allOpen
      onAll={() => {}}
      onStage={(p, un) => calls.push(`stage ${p} ${un}`)}
      onHunkStage={(p, i, k, un) => calls.push(`hunk ${p} ${i} ${k} ${un}`)}
      onRevertHunk={(p, i, k) => calls.push(`revert ${p} ${i} ${k}`)}
      commit={{ count: '1 staged', countTitle: 'staged files', placeholder: 'Fix the sum', onCommit: (m) => calls.push(`commit ${m}`) }}
    />
  ));
  const stage = host.querySelector<HTMLButtonElement>('[data-part~="panel.changes.stage"]');
  expect(stage?.hasAttribute('data-part-staged')).toBe(true); // part-staged
  stage?.click();
  const hunkStage = host.querySelectorAll<HTMLButtonElement>('[data-part~="panel.changes.stageHunk"]');
  expect([...hunkStage].map((b) => b.textContent)).toEqual(['Unstage', 'Stage']);
  hunkStage[1]?.click();
  host.querySelectorAll<HTMLButtonElement>('[data-part~="panel.changes.revertHunk"]')[0]?.click();
  host.querySelector<HTMLButtonElement>('[data-part~="panel.changes.viewed"]')?.click();
  const input = host.querySelector<HTMLInputElement>('[data-part~="panel.changes.commit"] input');
  expect(input?.placeholder).toBe('Fix the sum');
  host.querySelector<HTMLButtonElement>('[data-kit="cx-cm-go"]')?.click(); // empty: the placeholder is the message
  const [h0, h1] = hunkRuns(ROWS);
  expect(calls).toEqual([
    'stage src/math.js false', // part-staged: Stage stages the rest
    `hunk src/math.js 1 ${hunkKey(h1?.rows ?? [])} false`,
    `revert src/math.js 0 ${hunkKey(h0?.rows ?? [])}`,
    'viewed src/math.js',
    'commit Fix the sum',
  ]);
});

test('a hunk being reverted is hidden at once, and the file without staging has no stage controls', () => {
  const [going, setGoing] = createSignal<ReadonlySet<string>>(new Set());
  const host = mount(() => (
    <ChangesList
      files={[UNSTAGED]}
      labels={L}
      open={() => true}
      viewed={() => false}
      onToggle={() => {}}
      onViewed={() => {}}
      allOpen
      onAll={() => {}}
      onRevertHunk={() => {}}
      going={going()}
    />
  ));
  expect(host.querySelector('[data-part~="panel.changes.stage"]')).toBeNull();
  expect(host.querySelector('[data-part~="panel.changes.stageHunk"]')).toBeNull();
  expect(host.querySelector('[data-part~="panel.changes.commit"]')).toBeNull();
  const shown = (): number => [...host.querySelectorAll('[data-kit="cx-hunk"]')].filter((h) => !(h as HTMLElement).hidden).length;
  expect(shown()).toBe(2);
  setGoing(new Set(['src/math.js#0']));
  expect(shown()).toBe(1);
});

test('F-page-41: a .env file has v1 env-mask\'s line over its diff, Reveal ⇄ Hide in place, the button the same size', () => {
  const calls: string[] = [];
  const [env, setEnv] = createSignal<'hidden' | 'shown'>('hidden');
  const host = mount(() => (
    <ChangesList
      files={[{ ...FILE, path: 'app/.env.local', env: env() }, { ...FILE, path: 'src/a.ts' }]}
      labels={{ ...L, envHidden: (n) => `${n}’s values are hidden`, envShown: (n) => `Showing ${n}’s values`, reveal: 'Reveal', hide: 'Hide' }}
      open={() => true}
      viewed={() => false}
      onToggle={() => {}}
      onViewed={() => {}}
      allOpen
      onAll={() => {}}
      onReveal={(p, on) => calls.push(`${p} ${on}`)}
    />
  ));
  const bars = host.querySelectorAll('[data-part="changes.envMask"]');
  expect(bars.length).toBe(1); // only the .env file
  expect(bars[0]?.textContent).toBe('.env.local’s values are hiddenReveal');
  const btn = (): HTMLButtonElement => host.querySelector('[data-part="changes.envReveal"]') as HTMLButtonElement;
  expect(btn().getAttribute('aria-pressed')).toBe('false');
  btn().click();
  setEnv('shown');
  expect(btn().textContent).toBe('Hide');
  expect(btn().getAttribute('aria-pressed')).toBe('true');
  expect(host.querySelector('[data-kit="env-mask-what"]')?.textContent).toBe('Showing .env.local’s values');
  btn().click();
  expect(calls).toEqual(['app/.env.local true', 'app/.env.local false']);
});

// audit E #4: the list mounted on the edits wisp saw, then git's diff came; its labels were read once, so the header kept
// "edits wisp saw" over git's own rows. The words follow what the list is.
test('the_summary_words_follow_the_labels_as_they_change_audit_e_4', () => {
  const [vs, setVs] = createSignal('edits wisp saw');
  const file: ChangeFile = { path: 'math.js', status: 'M', add: 1, del: 1, binary: false, revertable: false, rows: ROWS.slice(0, 3) };
  const host = mount(() => (
    <ChangesList files={[file]} labels={{ ...L, vs: vs(), vsTitle: vs() }} open={() => true} viewed={() => false} onToggle={() => {}} onViewed={() => {}} allOpen onAll={() => {}} />
  ));
  const said = (): string | null | undefined => host.querySelector('[data-kit="cx-vs"]')?.textContent;
  expect(said()).toBe('edits wisp saw');
  setVs('vs origin/main');
  expect(said()).toBe('vs origin/main');
  expect(host.querySelector('[data-kit="cx-vs"]')?.getAttribute('title')).toBe('vs origin/main');
});

// audit E #10: two edits wisp saw in one file showed "⋯ 0 unchanged lines" between them. v1 changes.js drew a plain
// dashed separator (`{op: 'sep'}`, blocks.css tr.sep): an edit is a snippet, so nothing between is counted.
test('the_edits_wisp_saw_are_split_by_a_plain_separator_never_0_unchanged_lines_audit_e_10', () => {
  const rows: ChangeRow[] = [{ op: '-', text: 'a - b' }, { op: '+', text: 'a + b' }, { op: 'sep' }, { op: '-', text: 'x' }, { op: '…', count: 0 }, { op: '+', text: 'y' }];
  const file: ChangeFile = { path: 'math.js', status: 'M', add: 2, del: 2, binary: false, revertable: false, rows };
  const host = mount(() => <ChangesList files={[file]} labels={L} open={() => true} viewed={() => false} onToggle={() => {}} onViewed={() => {}} allOpen onAll={() => {}} />);
  expect(host.querySelectorAll('[data-kit="cx-sep"]').length).toBe(2);
  expect(host.querySelectorAll('[data-kit="cx-gap"]').length).toBe(0);
  expect(host.textContent).not.toContain('0 unchanged');
});

// audit E #3: on a phone the + sat over the first character of the diff's line ("+new file"). v1 comments.css: "the
// number is the target on touch": a tap on a line's number comments on it, and the + is drawn only under a pointer.
test('a_lines_number_comments_on_it_the_target_on_touch_audit_e_3', () => {
  const added: string[] = [];
  const file: ChangeFile = { path: 'math.js', status: 'M', add: 1, del: 1, binary: false, revertable: false, rows: ROWS.slice(0, 3) };
  const host = mount(() => (
    <ChangesList
      files={[file]}
      labels={L}
      open={() => true}
      viewed={() => false}
      onToggle={() => {}}
      onViewed={() => {}}
      allOpen
      onAll={() => {}}
      comment={{ label: () => 'Comment', onAdd: (p, r) => added.push(`${p} ${r.text}`), marks: () => new Map() }}
    />
  ));
  const nos = [...host.querySelectorAll<HTMLElement>('[data-kit="cx-no"][data-cmt]')];
  expect(nos.length).toBe(3);
  nos[2]?.click();
  expect(added).toEqual(['math.js   return a + b;']);
});

test('large_files_cap_rows_and_call_comment_marks_once_per_file_not_per_line', () => {
  const bigRows: ChangeRow[] = Array.from({ length: 650 }, (_, i) => ({ op: '+', text: `line ${i + 1}`, b: i + 1 }));
  const file: ChangeFile = { path: 'big.ts', status: 'M', add: 650, del: 0, binary: false, revertable: true, rows: bigRows };
  let marksCalls = 0;
  const host = mount(() => (
    <ChangesList
      files={[file]}
      labels={{ ...L, showMore: (n) => `Show ${n} more lines` }}
      open={() => true}
      viewed={() => false}
      onToggle={() => {}}
      onViewed={() => {}}
      allOpen
      onAll={() => {}}
      onRevertHunk={() => {}}
      comment={{
        label: () => 'Comment',
        onAdd: () => {},
        marks: () => {
          marksCalls += 1;
          return new Map();
        },
      }}
    />
  ));
  expect(host.querySelectorAll('[data-kit="cx-line"]').length).toBe(300);
  // a hunk cut short by the 300-row cap hides Revert until expanded
  expect(host.querySelector('[data-part="panel.changes.revertHunk"]')).toBeNull();
  expect(marksCalls).toBe(1);
  const more = host.querySelector<HTMLButtonElement>('[data-part="panel.changes.moreRows"]');
  expect(more?.textContent).toBe('Show 350 more lines');
  more?.click();
  expect(host.querySelectorAll('[data-kit="cx-line"]').length).toBe(650);
  expect(host.querySelector('[data-part="panel.changes.moreRows"]')).toBeNull();
  expect(host.querySelector('[data-part="panel.changes.revertHunk"]')).not.toBeNull();
  expect(marksCalls).toBe(1);
});

// perf-r1 #3: Show more lines revealed the whole diff in one mount (10,249 <tr>, ~0.5 s for a 10k-line file); it now
// adds a step at a time, saying how many are left, until every line shows
test('show_more_lines_reveals_a_huge_diff_in_steps_until_every_line_shows', () => {
  const rows: ChangeRow[] = Array.from({ length: 10_000 }, (_, i) => ({ op: '+', text: `line ${i + 1}`, b: i + 1 }));
  const file: ChangeFile = { path: 'huge.ts', status: 'M', add: 10_000, del: 0, binary: false, revertable: true, rows };
  const host = mount(() => (
    <ChangesList
      files={[file]}
      labels={{ ...L, showMore: (n, left) => (left > n ? `Show ${n} more lines (${left} left)` : `Show ${n} more lines`) }}
      open={() => true}
      viewed={() => false}
      onToggle={() => {}}
      onViewed={() => {}}
      allOpen
      onAll={() => {}}
      onRevertHunk={() => {}}
    />
  ));
  const more = (): HTMLButtonElement | null => host.querySelector<HTMLButtonElement>('[data-part="panel.changes.moreRows"]');
  const lines = (): number => host.querySelectorAll('[data-kit="cx-line"]').length;
  expect(lines()).toBe(300);
  expect(more()?.textContent).toBe('Show 1000 more lines (9700 left)');
  const at = performance.now();
  more()?.click();
  const ms = performance.now() - at;
  expect(host.querySelectorAll('tr').length).toBeLessThanOrEqual(1_300 + 2);
  expect(lines()).toBe(1_300);
  expect(ms).toBeLessThan(150);
  // the rows already shown stay where they are: only the new ones mount
  const first = host.querySelector('[data-kit="cx-line"]');
  more()?.click();
  expect(host.querySelector('[data-kit="cx-line"]')).toBe(first);
  for (let i = 0; i < 20 && more(); i++) more()?.click();
  expect(lines()).toBe(10_000);
  expect(more()).toBeNull();
  expect(host.querySelector('[data-part="panel.changes.revertHunk"]')).not.toBeNull();
});

test('file_rows_key_by_path_so_updating_files_patches_in_place_without_remounting_diffs', () => {
  const [files, setFiles] = createSignal<ChangeFile[]>([FILE]);
  const host = mount(() => (
    <ChangesList files={files()} labels={L} open={() => true} viewed={() => false} onToggle={() => {}} onViewed={() => {}} allOpen onAll={() => {}} />
  ));
  const rowNode = host.querySelector('[data-kit="cx-file"]');
  const tableNode = host.querySelector('[data-kit="cx-diff"] table');
  // fresh object reference with flipped stage state (as ChangesPane's memo produces)
  setFiles([{ ...FILE, stage: 'staged' }]);
  expect(host.querySelector('[data-kit="cx-file"]')).toBe(rowNode);
  expect(host.querySelector('[data-kit="cx-diff"] table')).toBe(tableNode);
  expect(host.querySelector('[data-part~="panel.changes.stage"]')?.getAttribute('aria-pressed')).toBe('true');
});

test('many_files_cap_initial_rows_and_unfold_in_batches', () => {
  const many: ChangeFile[] = Array.from({ length: 250 }, (_, i) => ({
    path: `src/f${i}.ts`,
    status: 'M',
    add: 1,
    del: 0,
    binary: false,
    revertable: false,
    rows: [{ op: '+', text: 'x', b: 1 }],
  }));
  const host = mount(() => (
    <ChangesList
      files={many}
      labels={{ ...L, moreFiles: (n) => `Show ${n} more files` }}
      open={() => false}
      viewed={() => false}
      onToggle={() => {}}
      onViewed={() => {}}
      allOpen={false}
      onAll={() => {}}
    />
  ));
  expect(host.querySelectorAll('[data-kit="cx-file"]').length).toBe(200);
  const more = host.querySelector<HTMLButtonElement>('[data-part="panel.changes.moreFiles"]');
  expect(more?.textContent).toBe('Show 50 more files');
  more?.click();
  expect(host.querySelectorAll('[data-kit="cx-file"]').length).toBe(250);
  expect(host.querySelector('[data-part="panel.changes.moreFiles"]')).toBeNull();
});
