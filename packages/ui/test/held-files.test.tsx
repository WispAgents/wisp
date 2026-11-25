// HeldFiles (the Done dialog's uncommitted files): git's porcelain code read as one badge, the path split folder
// / name, "+N more" past what the server sent; FoldToggle says whether it's open.
import { describe, expect, it } from 'vitest';
import { render } from 'solid-js/web';
import { FoldToggle, HeldFiles, heldKind } from '../src/content/HeldFiles.tsx';

const labels = { kind: (k: string): string => k, more: (n: number): string => `+${n} more` };

function mount(el: () => import('solid-js').JSX.Element): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  render(el, host);
  return host;
}

describe('HeldFiles', () => {
  it('porcelain_codes_read_as_one_change_untracked_conflict_index_first', () => {
    expect(['??', 'A.', '.M', 'M.', 'MM', '.D', 'D.', 'R.', 'C.', 'UU', 'AA', 'DD', '.T'].map(heldKind)).toEqual([
      'new', 'add', 'mod', 'mod', 'mod', 'del', 'del', 'ren', 'ren', 'conflict', 'conflict', 'conflict', 'mod',
    ]);
  });

  it('lists_each_file_folder_faint_name_strong_and_says_how_many_more', () => {
    const host = mount(() => (
      <HeldFiles files={[{ path: 'src/app/a.ts', xy: '.M' }, { path: 'notes.md', xy: '??' }, { path: 'b.ts', xy: 'R.', from: 'old/b.ts' }]} total={5} labels={labels} />
    ));
    const rows = [...host.querySelectorAll('[data-kit="held-file"]')];
    expect(rows.map((r) => r.querySelector('[data-kit="held-badge"]')?.textContent)).toEqual(['M', 'U', 'R']);
    expect(rows[0]?.querySelector('[data-kit="held-dir"]')?.textContent).toBe('src/app/');
    expect(rows[0]?.querySelector('[data-kit="held-base"]')?.textContent).toBe('a.ts');
    expect(rows[1]?.querySelector('[data-kit="held-dir"]')).toBeNull();
    expect(rows[2]?.querySelector('[data-kit="held-from"]')?.textContent).toBe('old/b.ts\u00a0→\u00a0');
    expect(host.querySelector('[data-kit="held-more"]')?.textContent).toBe('+2 more');
    host.remove();
  });

  it('an_untracked_folder_keeps_its_slash_on_the_name', () => {
    const host = mount(() => <HeldFiles files={[{ path: 'out/cache/', xy: '??' }]} labels={labels} />);
    expect(host.querySelector('[data-kit="held-dir"]')?.textContent).toBe('out/');
    expect(host.querySelector('[data-kit="held-base"]')?.textContent).toBe('cache/');
    expect(host.querySelector('[data-kit="held-more"]')).toBeNull();
    host.remove();
  });

  it('fold_toggle_says_open_and_toggles', () => {
    let n = 0;
    const host = mount(() => <FoldToggle label="3 uncommitted files" open={false} onToggle={() => n++} controls="x" />);
    const b = host.querySelector('button');
    expect(b?.getAttribute('aria-expanded')).toBe('false');
    b?.click();
    expect(n).toBe(1);
    host.remove();
  });
});
