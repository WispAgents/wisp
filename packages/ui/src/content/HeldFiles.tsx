// HeldFiles: the uncommitted files in a folder, read-only (the Done dialog's "what Remove would lose"). One
// line a file: git's change as a badge (the `files` block's colours) and its path, folder faint and name
// strong, a rename's old path before it. Past a few rows the list scrolls in place; past what the server sent,
// a last line says how many more. FoldToggle is the inline text button that opens it: its label, a chevron
// that turns, aria-expanded.
import { For, Show, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from './Icon.tsx';
import '../css/held-files.css';

/** git's change, from porcelain's two letters: what the badge says and how it's coloured. */
export type HeldKind = 'add' | 'mod' | 'del' | 'ren' | 'new' | 'conflict';

export interface HeldFile {
  readonly path: string;
  /** git's porcelain code: `.M`, `A.`, `??`, `UU` */
  readonly xy: string;
  readonly from?: string;
}

const MARK: Record<HeldKind, string> = { add: 'A', mod: 'M', del: 'D', ren: 'R', new: 'U', conflict: '!' };

/** `??` untracked, either side `U` (or `AA`/`DD`) a conflict, else the index's letter, else the worktree's. */
export function heldKind(xy: string): HeldKind {
  if (xy === '??') return 'new';
  if (xy.includes('U') || xy === 'AA' || xy === 'DD') return 'conflict';
  const c = xy[0] && xy[0] !== '.' && xy[0] !== ' ' ? xy[0] : xy[1];
  if (c === 'A') return 'add';
  if (c === 'D') return 'del';
  if (c === 'R' || c === 'C') return 'ren';
  return 'mod';
}

function split(p: string): readonly [string, string] {
  const trimmed = p.endsWith('/') ? p.slice(0, -1) : p;
  const i = trimmed.lastIndexOf('/');
  return i < 0 ? ['', p] : [p.slice(0, i + 1), p.slice(i + 1)];
}

export interface HeldFilesProps extends KitProps {
  readonly files: readonly HeldFile[];
  /** how many there are in all; past `files.length` the last line says "+N more" */
  readonly total?: number;
  readonly labels: {
    /** the badge's accessible name: Added, Modified, Untracked … */
    readonly kind: (k: HeldKind) => string;
    readonly more: (n: number) => string;
  };
}

export function HeldFiles(props: HeldFilesProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.held-files';
  const more = (): number => Math.max(0, (props.total ?? props.files.length) - props.files.length);
  return (
    <ul data-kit="held-files" data-part={part()} id={props.id}>
      <For each={props.files}>
        {(f) => {
          const kind = heldKind(f.xy);
          const [dir, base] = split(f.path);
          return (
            <li data-kit="held-file" data-part={sub(part(), 'file')} title={f.from ? `${f.from} → ${f.path}` : f.path}>
              <span data-kit="held-badge" data-change={kind} role="img" aria-label={props.labels.kind(kind)} title={props.labels.kind(kind)}>
                {MARK[kind]}
              </span>
              <span data-kit="held-path" data-text="control">
                <Show when={f.from}>
                  {(from) => <span data-kit="held-from">{`${from()}\u00a0→\u00a0`}</span>}
                </Show>
                <Show when={dir}>
                  <span data-kit="held-dir">{dir}</span>
                </Show>
                <span data-kit="held-base">{base}</span>
              </span>
            </li>
          );
        }}
      </For>
      <Show when={more() > 0}>
        <li data-kit="held-more" data-part={sub(part(), 'more')} data-text="control">
          {props.labels.more(more())}
        </li>
      </Show>
    </ul>
  );
}

export interface FoldToggleProps extends KitProps {
  readonly open: boolean;
  readonly onToggle: () => void;
  readonly label: string;
  /** warn: what it opens would be lost */
  readonly tone?: 'muted' | 'warn';
  /** the id of what it opens */
  readonly controls?: string;
  readonly title?: string;
}

/** An inline text button that opens what's under it: the label, then a chevron that turns down when open. */
export function FoldToggle(props: FoldToggleProps): JSX.Element {
  return (
    <button
      type="button"
      id={props.id}
      data-kit="fold-toggle"
      data-part={props.part ?? 'ui.fold-toggle'}
      data-tone={props.tone ?? 'muted'}
      data-open={flag(props.open)}
      aria-expanded={props.open}
      aria-controls={props.controls}
      title={props.title}
      onClick={() => props.onToggle()}
    >
      <span data-text="control">{props.label}</span>
      <Icon name="chevron" size="sm" />
    </button>
  );
}
