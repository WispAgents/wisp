// Icon: one of the set in icons.ts, drawn in currentColor at a size from the scale. The first screen's icons
// (icons-first.ts) come with the page; the rest of the set loads after the first screen (ADR-0105) or when one of
// them is first drawn, and joins the sprite: its <use> draws once its symbol is there.
import type { JSX } from 'solid-js';
import type { IconName } from './icons.ts';
import { FIRST_ICONS } from './icons-first.ts';
import { afterFirstScreen } from '../controls/lazy-part.ts';
import type { KitProps } from '../core/props.ts';
import type { TextTone } from './Text.tsx';

export type IconSize = 'xs' | 'sm' | 'md' | 'base' | 'lg';

export interface IconProps extends KitProps {
  readonly name: IconName;
  /** xs 11, sm 12, md 14, base 16 (the default), lg 18. */
  readonly size?: IconSize;
  readonly tone?: TextTone;
  /** An icon that means something on its own gets a name; a decorative one is hidden from readers. */
  readonly label?: string;
}

const SPRITE = 'k-icons';

/** The set as one sprite of <symbol>s, put in the document once (v1's index.html sprite): each icon is
 * then one <use>, so a page with hundreds of icons holds their shapes once. */
// The markup is the kit's own constant (icons.ts), never user data.
const symbols = (set: Readonly<Record<string, readonly [string, string]>>, skip?: Readonly<Record<string, unknown>>): string =>
  Object.entries(set)
    .filter(([name]) => !skip || !(name in skip))
    .map(([name, [box, markup]]) => `<symbol id="k-i-${name}" viewBox="${box}">${markup}</symbol>`)
    .join('');

/** the sprite in the document, the rest of the set once it's loaded, and its fetch (once; a failed fetch is tried
 *  again on the next draw) */
const set: { sprite?: SVGSVGElement; rest?: Readonly<Record<string, readonly [string, string]>>; loading?: Promise<void> | undefined } = {};
function ensureSprite(): void {
  if (set.sprite?.isConnected || typeof document === 'undefined') return;
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.id = SPRITE;
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('style', 'position:absolute;width:0;height:0;overflow:hidden');
  svg.innerHTML = symbols(FIRST_ICONS);
  document.getElementById(SPRITE)?.remove();
  document.body.prepend(svg);
  set.sprite = svg;
  if (set.rest) svg.insertAdjacentHTML('beforeend', symbols(set.rest, FIRST_ICONS));
}

export function loadIcons(): Promise<void> {
  return (set.loading ??= import('./icons.ts').then(
    (m) => {
      set.rest = m.ICONS;
      if (set.sprite?.isConnected) set.sprite.insertAdjacentHTML('beforeend', symbols(m.ICONS, FIRST_ICONS));
    },
    (err: unknown) => {
      set.loading = undefined;
      throw err;
    },
  ));
}
afterFirstScreen(() => {
  loadIcons().catch(() => undefined);
});

export function Icon(props: IconProps): JSX.Element {
  ensureSprite();
  const name = (): IconName => {
    const n = props.name;
    if (!set.rest && !(n in FIRST_ICONS)) loadIcons().catch(() => undefined);
    return n;
  };
  return (
    <svg
      id={props.id}
      // every icon in the set is drawn on a 24 grid (icons.ts); the symbol carries its own box too
      viewBox="0 0 24 24"
      role={props.label ? 'img' : undefined}
      aria-label={props.label}
      aria-hidden={props.label ? undefined : 'true'}
      data-kit="icon"
      data-part={props.part ?? 'ui.icon'}
      data-size={props.size ?? 'base'}
      data-tone={props.tone}
      data-icon={name()}
    >
      <use href={`#k-i-${name()}`} />
    </svg>
  );
}
