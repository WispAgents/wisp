// What a card on My Stuff's page shows of an item (v1 features/stuff.js paintCard, thumb, thumbBody, launchBtn, pubPill):
// its preview, its facts line, its version and Public pill, its note, tags and the task it came from. Pure: the page
// adds what clicks do, the tests read the same rules.
import './i18n/lazy.ts';

import type { IconName, StuffThumb } from '@wisp/ui';
import { kindOf } from './kinds.ts';
import { bare, behind, cardMeta, ext, texty, when, type Translate } from './model.ts';
import type { Item } from './types.ts';

export const fileUrl = (id: string, rev?: number, download = false): string => `/api/stuff/${encodeURIComponent(id)}/file?r=${rev ?? 1}${download ? '&download=1' : ''}`;
export const coverUrl = (id: string, rev?: number): string => `/api/stuff/${encodeURIComponent(id)}/cover?r=${rev ?? 1}`;

/** What a phone tile or row shows in its thumbnail: a picture's file or an app's cover, as the page's cards do;
 *  undefined keeps the kind's glyph (QA r2 phone #13: every tile was a glyph, even a picture). */
export function phoneImage(it: Item): string | undefined {
  const th = thumbOf(it);
  if (th.kind === 'image') return th.src;
  if (th.kind === 'app') return th.cover;
  return undefined;
}

/** The preview (v1 thumbBody): an app's window, the picture, a doc's opening, a snippet's first lines, a link's site. */
export function thumbOf(it: Item): StuffThumb {
  const k = kindOf(it.kind);
  if (it.kind === 'app') return { kind: 'app', name: it.name || 'app', cover: it.cover ? coverUrl(it.id, it.rev) : undefined, initial: (it.title || '?').trim().charAt(0).toUpperCase() || '?', excerpt: it.excerpt };
  if (it.pending && it.kind !== 'doc' && it.kind !== 'code') return { kind: 'pending', icon: k.icon };
  if (it.kind === 'image') return { kind: 'image', src: fileUrl(it.id, it.rev) };
  if (it.kind === 'video') return { kind: 'video', src: fileUrl(it.id, it.rev) };
  if (it.kind === 'doc' && texty(it)) return { kind: 'doc', markdown: (it.excerpt ?? '').slice(0, 700) };
  if (it.kind === 'code' && texty(it)) return { kind: 'code', text: it.excerpt ?? '', lang: it.lang || ext(it).toLowerCase() };
  if (it.kind === 'link') {
    let path = '';
    try {
      const u = new URL(it.url ?? '');
      path = (u.pathname + u.search).replace(/^\/$/, '');
    } catch {
      path = '';
    }
    return { kind: 'link', site: it.name, path };
  }
  return { kind: 'ext', label: ext(it) || 'FILE' };
}

const full = (at: number): string => new Date(at).toLocaleString(undefined, { weekday: 'short', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
const short = (at: number): string => new Date(at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });

export interface CardLook {
  readonly icon: IconName;
  readonly label: string;
  readonly facts: string;
  readonly time: { label: string; title: string };
  readonly version?: { label: string; title: string };
  readonly pub?: { label: string; title?: string; old?: boolean; busy?: boolean };
  readonly openData?: string;
}

/** The card's words: kind, facts, time, version, Public. */
export function cardLook(it: Item, t: Translate, now: number): CardLook {
  const k = kindOf(it.kind);
  const look: CardLook = {
    icon: k.icon,
    label: t('stuff.card.label', { kind: t(k.one), title: it.title }),
    facts: cardMeta(it, t),
    time: { label: when(it.at, now), title: full(it.at) },
    ...((it.rev ?? 1) > 1 && { version: { label: `v${it.rev}`, title: t(it.prev ? 'stuff.card.updatedPrev' : 'stuff.card.updated', { when: short(it.updatedAt ?? it.at) }) } }),
    ...(it.publishing
      ? { pub: { label: t('stuff.card.publishing'), busy: true } }
      : it.published
        ? { pub: { label: t('stuff.card.public'), old: behind(it), title: t(behind(it) ? 'stuff.card.publicOld' : 'stuff.card.publicTitle', { at: bare(it.published.url) }) } }
        : {}),
    ...(it.published && it.db && { openData: t('stuff.card.openData') }),
  };
  return look;
}

/** An app's main action on its preview: Launch, or Running / Starting… while it runs. */
export function launchLook(it: Item, t: Translate): { label: string; title: string; running: boolean; dot: boolean } | undefined {
  if (it.kind !== 'app' || it.pending) return undefined;
  if (!it.running) return { label: t('stuff.card.launch'), title: t('stuff.card.launchTitle'), running: false, dot: false };
  const up = it.running.port > 0;
  return { label: t(up ? 'stuff.card.running' : 'stuff.card.starting'), title: t('stuff.card.runningTitle'), running: true, dot: up };
}

/** "6 things · 81 KB" (v1 paintTop). */
export function headCount(items: readonly Item[], t: Translate, size: (n: number) => string): string {
  if (!items.length) return '';
  const bytes = items.reduce((n, x) => n + (x.size || 0), 0);
  const count = t('stuff.page.count', { count: items.length });
  return bytes ? t('stuff.page.sizeCount', { count, size: size(bytes) }) : count;
}

/** What nothing matched: the words, the kind, starred (v1 emptySearch). */
export function noneWhat(q: string, kind: string, starred: boolean, t: Translate): string {
  return [q && `“${q}”`, kind !== 'all' && t(kindOf(kind).many).toLowerCase(), starred && t('stuff.page.noneStarred')].filter(Boolean).join(', ');
}
