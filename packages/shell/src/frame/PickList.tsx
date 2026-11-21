// The picker "All goals…" and "All recipes…" open (./pick-list.ts): the palette's sheet over every row, filtered as you
// type by name and second line (the menus' own match), ↑ ↓ to move, Enter or a click to pick, Esc to close.
import { createMemo, createSignal, onMount, type JSX } from 'solid-js';
import type { ExtensionId, PluginClient } from '@wisp/client';
import { matchesQuery, PaletteSheet, type PaletteRow } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { PICK_VIEW, pickDone, pickSpec } from './pick-list.ts';
import '../i18n/lazy.ts';

export default function PickList(props: { readonly client: PluginClient; readonly params: { readonly key: string } }): JSX.Element {
  const shell = useShell();
  const spec = pickSpec(props.params.key);
  const close = (): void => shell.navigate({ t: 'closeOverlay', id: PICK_VIEW as ExtensionId });
  // reopened after a reload (its rows were this page's): nothing to show
  onMount(() => !spec && close());
  const [q, setQ] = createSignal('');
  const [sel, setSel] = createSignal(0);
  const shown = createMemo(() => (spec?.rows ?? []).filter((r) => matchesQuery(r, q())));
  const rows = createMemo((): PaletteRow[] => shown().map((r) => ({ id: r.value, label: [[r.label, false]], ...(r.hint && { sub: r.hint }), icon: 'target' as const })));
  const pick = (i: number): void => {
    const r = shown()[i];
    if (!r || !spec) return;
    pickDone(props.params.key);
    close();
    spec.onPick(r.value);
  };
  const onKey = (e: KeyboardEvent): void => {
    const n = shown().length;
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (n) setSel((s) => (s + (e.key === 'ArrowDown' ? 1 : n - 1)) % n);
      document.getElementById(`palette-row-${sel()}`)?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'Enter' && !e.isComposing) {
      e.preventDefault();
      pick(sel());
    }
  };
  return (
    <PaletteSheet
      part="shell.pick"
      variant="issues"
      value={q()}
      placeholder={spec?.placeholder ?? ''}
      label={spec?.title ?? ''}
      rows={rows()}
      selected={sel()}
      empty={q().trim() ? shell.t('shell.pick.empty', { q: q().trim() }) : shell.t('shell.pick.none')}
      foot={{ navigate: shell.t('shell.pick.navigate'), open: shell.t('shell.pick.open'), close: shell.t('shell.pick.close') }}
      close={{ back: shell.t('shell.phone.back'), cancel: shell.t('shell.phone.cancel'), onClose: close }}
      onInput={(v) => {
        setQ(v);
        setSel(0);
      }}
      onKeyDown={onKey}
      onPick={pick}
      onHover={setSel}
      inputRef={(el) => requestAnimationFrame(() => el.focus())}
    />
  );
}
