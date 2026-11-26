// A prompt box's @ and / menus (v1 composer.js, mention-text.js, slash-text.js): which menu the caret is in and what
// it's looking for, what a pick does to the text, and how the box's `@Title`s are sent (`@[Title](scheme:id)`).
// Pure; `usePromptMenu` wires it to a box, the plugins' sources (contract PromptSourceDef) and the kit's list.
import { createMemo, createSignal, type Accessor } from 'solid-js';
import type { PromptSuggest } from '@wisp/ui';
import { useDerived } from '@wisp/ui/pure';
import { ownerOf, type Json } from '@wisp/client';
import type { PromptItem, PromptSourceDef, ShellContext } from '../contract.ts';

export interface MenuAt {
  readonly trigger: '@' | '/';
  /** where the trigger is: the pick replaces from here to the caret */
  readonly start: number;
  readonly query: string;
}

/** The menu the caret is in: `/word` at the very start (a command, v1), its one word after it (`/goal gre`: query
 *  `goal gre`, what a source suggests for the command's argument: owner-list #63), `@words` after a space or the start. */
export function menuAt(text: string, caret: number): MenuAt | null {
  const before = text.slice(0, caret);
  const slash = /^\/(\S*)$/.exec(before) ?? /^\/([a-z][\w-]* \S*)$/i.exec(before);
  if (slash) return { trigger: '/', start: 0, query: slash[1] ?? '' };
  const at = /(^|\s)@([^\s@]*)$/.exec(before);
  if (at) return { trigger: '@', start: caret - (at[2] ?? '').length - 1, query: at[2] ?? '' };
  return null;
}

/** The text after a pick: the trigger and what followed it replaced by the item's insert, the caret after it. */
export function applyPick(text: string, m: MenuAt, caret: number, item: Pick<PromptItem, 'insert'>): { text: string; caret: number } {
  const next = text.slice(0, m.start) + item.insert + text.slice(caret);
  return { text: next, caret: m.start + item.insert.length };
}

export interface Mentioned {
  readonly scheme: 'wisp' | 'crew' | 'stuff';
  readonly id: string;
  readonly title: string;
}

/** v1's stored form of one mention, `@[Title](scheme:id)`: a title without `]` or a line break, an id without `)` or a space */
const STORED = /@\[([^\]\n]+)\]\((wisp|crew|stuff):([^)\s]+)\)/g;
/** what may follow a whole `@Title` besides a space or the end: a sentence's punctuation (never a letter, digit, `-` or
 *  `'`: `@Authentic` and `@Auth-bug` are other words than `@Auth`) */
const ENDS = /[.,;:!?)\]}"]/;
const spansOf = (text: string): (readonly [number, number])[] => [...text.matchAll(STORED)].map((m) => [m.index, m.index + m[0].length] as const);

/**
 * What is sent: each picked `@Title` still in the text as `@[Title](scheme:id)` (v1's stored form); the rest as typed.
 * A `@Title` counts only as a whole word, where menuAt opens (the start or after a space) up to the end, a space or
 * punctuation, every time it's there; the longest title first and what is already stored left as it is, so picking
 * `Auth` and `Auth bug` never sends `@[Auth](…) bug` nor reads `@Auth` inside `@[Auth bug](…)` (doc25b).
 */
export function encodeMentions(text: string, mentions: readonly Mentioned[]): string {
  let out = text;
  for (const m of [...mentions].sort((a, b) => b.title.length - a.title.length)) {
    if (!m.title) continue;
    const shown = `@${m.title}`;
    const stored = spansOf(out);
    let next = '';
    let from = 0;
    for (let i = out.indexOf(shown); i >= 0; i = out.indexOf(shown, i + 1)) {
      const at = i;
      const end = at + shown.length;
      if (at < from || stored.some(([a, b]) => at >= a && at < b)) continue;
      if (at > 0 && !/\s/.test(out[at - 1] ?? '')) continue;
      if (end < out.length && !/\s/.test(out[end] ?? '') && !ENDS.test(out[end] ?? '')) continue;
      next += `${out.slice(from, at)}@[${m.title}](${m.scheme}:${m.id})`;
      from = end;
    }
    out = next + out.slice(from);
  }
  return out;
}

/**
 * The box's form of sent text (doc25a): each `@[Title](scheme:id)` back to the `@Title` the box shows, with its mention
 * (in order, once each), so a prompt put back (↑'s history, a refused send) sends its mentions again. Anything else,
 * half-written markup included, stays as it is. `decodeMentions(encodeMentions(t, ms))` gives back `t` and `ms`.
 */
export function decodeMentions(text: string): { text: string; mentions: Mentioned[] } {
  const mentions: Mentioned[] = [];
  const out = text.replace(STORED, (_, title: string, scheme: Mentioned['scheme'], id: string) => {
    if (!mentions.some((m) => m.scheme === scheme && m.id === id && m.title === title)) mentions.push({ scheme, id, title });
    return `@${title}`;
  });
  return { text: out, mentions };
}

/** The items for a menu: every source for its trigger and box, in order, at most `max`. */
export function itemsFor(sources: readonly PromptSourceDef[], m: MenuAt, where: 'reply' | 'home', ctx: Parameters<PromptSourceDef['items']>[1], max = 8): PromptItem[] {
  const out: PromptItem[] = [];
  for (const s of sources) {
    if (s.trigger !== m.trigger || (s.where && !s.where.includes(where))) continue;
    for (const it of s.items(m.query, ctx)) {
      out.push(it);
      if (out.length >= max) return out;
    }
  }
  return out;
}

/**
 * A paste that is only task ids (v1 mention-text.js pastedTasks: Copy task ID, then paste), each a task here: their
 * mentions, in order, once each; else null and the box takes the text as it is. `title(id)`: the task's title, or
 * undefined when there's no such task.
 */
export function pastedTasks(text: string, title: (id: string) => string | undefined): Mentioned[] | null {
  const parts = text.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0 || parts.length > 10) return null;
  const out: Mentioned[] = [];
  for (const p of parts) {
    const id = /^(?:wisp:)?([\w:-]{4,80})$/.exec(p)?.[1] ?? /#\/s\/([\w:-]{4,80})\/?$/.exec(p)?.[1];
    const t = id ? title(id) : undefined;
    if (!id || t === undefined) return null;
    if (!out.some((m) => m.id === id)) out.push({ scheme: 'wisp', id, title: t });
  }
  return out;
}

/** The input types a phone keyboard's clipboard (Gboard's chip, iOS's suggestion) puts text in with, no paste event. */
const INSERTS = new Set(['insertText', 'insertFromPaste', 'insertReplacementText']);

/**
 * What one input put in that is only task ids (v1 #800): `inserted` is the input's text (its `data`, or its
 * dataTransfer's), ending at `caret` in `text`. Typing adds a letter at a time, so only an insert of 12 characters or
 * more whole in one go counts (v1's bound); the ids' mentions and the span they replace, else null.
 */
export function insertedTasks(text: string, caret: number, inserted: string, title: (id: string) => string | undefined): { from: number; to: number; got: Mentioned[] } | null {
  if (inserted.trim().length < 12) return null;
  const from = caret - inserted.length;
  if (from < 0 || text.slice(from, caret) !== inserted) return null;
  const got = pastedTasks(inserted, title);
  return got ? { from, to: caret, got } : null;
}

/** The box after its span `from`..`to` becomes the mentions' `@Title`s, a space before them after a word (v1 put). */
export function putMentions(text: string, from: number, to: number, got: readonly Mentioned[]): { text: string; caret: number } {
  const lead = from > 0 && !/\s$/.test(text.slice(0, from)) ? ' ' : '';
  const words = `${lead}${got.map((m) => `@${m.title}`).join(' ')} `;
  return { text: `${text.slice(0, from)}${words}${text.slice(to)}`, caret: from + words.length };
}

/** The title of the task `id` names on `peer` (`peek` reads a session row by key), else undefined: a key of another
 *  computer's task is not one there (new1). */
export function titleOn(peer: string, id: string, peek: (key: string) => string | undefined): string | undefined {
  if (id.includes(':') && ownerOf(id) !== peer) return undefined;
  return peek(id.includes(':') ? id : `${peer}:${id}`);
}

/** Mentions kept in a draft (persisted: read as data, anything else dropped) */
const asMentions = (v: Json | undefined): readonly Mentioned[] =>
  Array.isArray(v)
    ? (v as unknown[]).filter((m): m is Mentioned => {
        const x = m as Partial<Mentioned> | null;
        return !!x && typeof x.id === 'string' && typeof x.title === 'string' && (x.scheme === 'wisp' || x.scheme === 'crew' || x.scheme === 'stuff');
      })
    : [];

export interface PromptMenu {
  readonly suggest: Accessor<PromptSuggest | undefined>;
  /** call from the box's input: what's typed and where the caret is; with its input event, task ids a phone
   *  keyboard's clipboard put in become their mentions (v1 #800). The box's text after it. */
  readonly typed: (text: string, e?: InputEvent) => string;
  /** what to send: the text with its mentions encoded */
  readonly encode: (text: string) => string;
  /** after a send: the mentions go */
  readonly reset: () => void;
  /** a sent prompt back in the box (↑'s history, a refused send): its stored mentions shown as `@Title` and kept, so
   *  sending it again sends them (doc25a). The box's text after it. */
  readonly restore: (stored: string) => string;
  /** a paste: task ids become their mentions (true: taken) */
  readonly paste: (e: ClipboardEvent) => boolean;
}

export function usePromptMenu(o: {
  readonly shell: ShellContext;
  readonly where: 'reply' | 'home';
  readonly task?: string;
  /** New task's picked project's folder (its files for @) */
  readonly project?: () => string | undefined;
  /** the computer the box sends to (New task's Run on): its tasks are the ones @ offers and a pasted id is read on
   *  (new1). A reply box's is its task's; unset, this wisp's own */
  readonly peer?: () => string | undefined;
  /** the box's draft (drafts.ts): its mentions are kept beside it, `<field>.mentions`, so a reload or a task switch
   *  keeps what its `@Title`s send (doc25a); unset (a test's box), they live as long as the box */
  readonly draft?: readonly [string, string];
  readonly text: Accessor<string>;
  readonly setText: (v: string) => void;
  readonly input: () => HTMLTextAreaElement | undefined;
  readonly id: string;
}): PromptMenu {
  const sources = o.shell.promptSources();
  const [at, setAt] = createSignal<MenuAt | null>(null);
  const [active, setActive] = createSignal(0);
  const [own, setOwn] = createSignal<readonly Mentioned[]>([]);
  const kept = o.draft && o.shell.client.drafts.get<Json>(o.draft[0], `${o.draft[1]}.mentions`);
  const mentions = (): readonly Mentioned[] => (kept ? asMentions(kept()) : own());
  const setMentions = (ms: readonly Mentioned[]): void => {
    if (o.draft) o.shell.client.drafts.set(o.draft[0], `${o.draft[1]}.mentions`, ms.length > 0 ? (ms as unknown as Json) : undefined);
    else setOwn(ms);
  };
  const base = { client: o.shell.client.plugin('shell'), ...(o.task !== undefined && { task: o.task }) };
  const peerNow = (): string => o.peer?.() ?? (o.task?.includes(':') ? ownerOf(o.task) : (o.shell.client.plugin('shell') as { home: string }).home);
  // New task's picked project and Run on, read when the menu asks (its files and its computer's tasks for @)
  const ctxNow = (): Parameters<PromptSourceDef['items']>[1] => {
    const project = o.project?.();
    return { ...base, peer: peerNow(), ...(project && { project }) };
  };
  // useDerived, not createMemo (round 324): the sources the registry holds and what each source's items read (the
  // entity store's tasks for @) are alien signals; a Solid memo read them once, so a task that landed, was renamed or
  // went while the menu showed, or a source registered later, never reached it
  const items = useDerived((): PromptItem[] => {
    const m = at();
    return m ? itemsFor(sources(), m, o.where, ctxNow()) : [];
  });
  const pick = (value: string): void => {
    const m = at();
    const it = items().find((x) => x.value === value);
    const el = o.input();
    if (!m || !it) return;
    const caret = el?.selectionStart ?? o.text().length;
    const r = applyPick(o.text(), m, caret, it);
    if (it.mention) setMentions([...mentions().filter((x) => x.title !== it.mention?.title), it.mention]);
    o.setText(r.text);
    setAt(null);
    queueMicrotask(() => {
      el?.focus();
      el?.setSelectionRange(r.caret, r.caret);
    });
  };
  const suggest = createMemo((): PromptSuggest | undefined => {
    const list = items();
    if (!at() || list.length === 0) return undefined;
    return {
      id: o.id,
      label: o.shell.t(at()?.trigger === '/' ? 'shell.prompt.slash' : 'shell.prompt.mention'),
      items: list.map((x) => ({
        value: x.value,
        label: x.label,
        ...(x.detail && { detail: x.detail }),
        ...(x.hint && { hint: x.hint }),
        ...(x.icon && { icon: x.icon as never }),
        ...(x.open && { onOpen: x.open, openLabel: o.shell.t('shell.prompt.open', { name: x.label }) }),
      })),
      active: Math.min(active(), list.length - 1),
      onPick: pick,
      onActive: setActive,
      onClose: () => setAt(null),
    };
  });
  // a pasted id is a task on the computer the box sends to: the reply reads its mentions there (runtime mentions.rs),
  // so another computer's task would be dropped unsaid (new1)
  const titleOf = (id: string): string | undefined => titleOn(peerNow(), id, (key) => o.shell.client.entities.peek<{ title?: string }>('session', key as never)?.title);
  // the span becomes the mentions' @Titles, the caret after them
  const put = (text: string, from: number, to: number, got: readonly Mentioned[]): string => {
    const el = o.input();
    const r = putMentions(text, from, to, got);
    setMentions([...mentions().filter((x) => !got.some((g) => g.title === x.title)), ...got]);
    o.setText(r.text);
    setAt(null);
    queueMicrotask(() => el?.setSelectionRange(r.caret, r.caret));
    return r.text;
  };
  return {
    suggest,
    typed: (text, e) => {
      const caret = o.input()?.selectionStart ?? text.length;
      // a phone keyboard's clipboard puts a copied task id in like typing, with no paste event (v1 #800)
      if (e && INSERTS.has(e.inputType) && !e.isComposing) {
        const r = insertedTasks(text, caret, e.data ?? e.dataTransfer?.getData('text/plain') ?? '', titleOf);
        if (r) return put(text, r.from, r.to, r.got);
      }
      setAt(menuAt(text, caret));
      setActive(0);
      return text;
    },
    encode: (text) => encodeMentions(text, mentions()),
    reset: () => setMentions([]),
    restore: (stored) => {
      const back = decodeMentions(stored);
      setMentions([...mentions().filter((x) => !back.mentions.some((g) => g.title === x.title)), ...back.mentions]);
      o.setText(back.text);
      return back.text;
    },
    paste: (e) => {
      const got = pastedTasks(e.clipboardData?.getData('text/plain') ?? '', titleOf);
      if (!got) return false;
      e.preventDefault();
      const el = o.input();
      const text = o.text();
      const from = el?.selectionStart ?? text.length;
      put(text, from, el?.selectionEnd ?? from, got);
      return true;
    },
  };
}
