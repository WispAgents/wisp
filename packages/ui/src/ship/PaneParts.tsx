// The PR pane's other rows (design/side-panel.md PR), drawn on the same grid as CheckRow so the checks, their fold,
// the files and the "and n more" line share one left edge: a group head (title · a quiet count), a fold row, and a
// file row (folder gives way first, the name stays, +n −n in the diff colours like the Changes pane).
import { Show, type JSX } from 'solid-js';
import type { KitProps, WithChildren } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import '../css/ship.css';

export interface PaneHeadProps extends KitProps {
  readonly title: string;
  /** a quiet count beside the title ("4", "+633 −17") */
  readonly meta?: JSX.Element;
}

export function PaneHead(props: PaneHeadProps): JSX.Element {
  return (
    <div id={props.id} data-kit="pane-head" data-part={props.part ?? 'ui.pane-head'}>
      <h4 data-kit="pane-head-title">{props.title}</h4>
      <Show when={props.meta}><span data-kit="pane-head-meta">{props.meta}</span></Show>
    </div>
  );
}

export interface FoldRowProps extends KitProps {
  readonly label: string;
  /** a leading icon in the rows' icon column; none lines the words up with rows that have no icon (files) */
  readonly icon?: IconName;
  readonly tone?: 'good' | 'muted';
  readonly expanded?: boolean;
  readonly onClick: () => void;
}

/** A row-shaped button under a list ("2 passed · 1 skipped", "and 13 more"), aligned to the rows above it; its
 *  trailing chevron points down while shut, up once open (v1 .pn-fold). */
export function FoldRow(props: FoldRowProps): JSX.Element {
  return (
    <button
      type="button"
      id={props.id}
      data-kit="fold-row"
      data-part={props.part ?? 'ui.fold-row'}
      data-tone={props.tone ?? 'muted'}
      aria-expanded={props.expanded}
      onClick={() => props.onClick()}
    >
      <Show when={props.icon}>{(i) => <span data-kit="fold-row-icon"><Icon name={i()} size="sm" /></span>}</Show>
      <span data-kit="fold-row-label">{props.label}</span>
      <span data-kit="fold-row-chev"><Icon name="chevron" size="sm" /></span>
    </button>
  );
}

export function FileStatList(props: KitProps & WithChildren & { readonly label?: string }): JSX.Element {
  return (
    <ul id={props.id} data-kit="file-stat-list" data-part={props.part ?? 'ui.file-stat-list'} aria-label={props.label}>
      {props.children}
    </ul>
  );
}

export interface FileStatRowProps extends KitProps {
  readonly path: string;
  readonly additions: number;
  readonly deletions: number;
  readonly href?: string;
}

export function FileStatRow(props: FileStatRowProps): JSX.Element {
  const cut = (): number => props.path.lastIndexOf('/');
  const body = (): JSX.Element => (
    <>
      <span data-kit="file-stat-path">
        <Show when={cut() >= 0}>
          <span data-kit="file-stat-dir"><bdi>{props.path.slice(0, cut() + 1)}</bdi></span>
        </Show>
        <span data-kit="file-stat-base">{props.path.slice(cut() + 1)}</span>
      </span>
      <span data-kit="file-stat-n" data-op="add">{props.additions ? `+${props.additions}` : ''}</span>
      <span data-kit="file-stat-n" data-op="del">{props.deletions ? `−${props.deletions}` : ''}</span>
    </>
  );
  return (
    <li id={props.id} data-kit="file-stat-row" data-part={props.part ?? 'ui.file-stat-row'}>
      <Show when={props.href} fallback={<span data-kit="file-stat-link" title={props.path}>{body()}</span>}>
        {(h) => (
          <a data-kit="file-stat-link" href={h()} target="_blank" rel="noopener noreferrer" title={props.path}>
            {body()}
          </a>
        )}
      </Show>
    </li>
  );
}
