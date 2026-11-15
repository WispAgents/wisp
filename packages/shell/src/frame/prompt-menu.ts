// A prompt box's @ and / menus (v1 composer.js, mention-text.js, slash-text.js): which menu the caret is in and what
// it's looking for, what a pick does to the text, and how the box's `@Title`s are sent (`@[Title](scheme:id)`).
// Pure; `usePromptMenu` wires it to a box, the plugins' sources (contract PromptSourceDef) and the kit's list.
import { createMemo, createSignal, type Accessor } from 'solid-js';
import type { PromptSuggest } from '@wisp/ui';
import type { PromptItem, PromptSourceDef, ShellContext } from '../contract.ts';

export interface MenuAt {
  readonly trigger: '@' | '/';
  /** where the trigger is: the pick replaces from here to the caret */
  readonly start: number;
  readonly query: string;
}

/** The menu the caret is in: `/word` at the very start (a command, v1), `@words` after a space or the start. */
export function menuAt(text: string, caret: number): MenuAt | null {
  const before = text.slice(0, caret);
  const slash = /^\/(\S*)$/.exec(before);
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

/** What is sent: each picked `@Title` still in the text as `@[Title](scheme:id)` (v1's stored form); the rest as typed. */
export function encodeMentions(text: string, mentions: readonly Mentioned[]): string {
  let out = text;
  for (const m of mentions) {
    const shown = `@${m.title}`;
    const i = out.indexOf(shown);
    if (i < 0) continue;
    out = `${out.slice(0, i)}@[${m.title}](${m.scheme}:${m.id})${out.slice(i + shown.length)}`;
  }
  return out;
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

export interface PromptMenu {
  readonly suggest: Accessor<PromptSuggest | undefined>;
  /** call from the box's input: what's typed and where the caret is; with its input event, task ids a phone
   *  keyboard's clipboard put in become their mentions (v1 #800). The box's text after it. */
  readonly typed: (text: string, e?: InputEvent) => string;
  /** what to send: the text with its mentions encoded */
  readonly encode: (text: string) => string;
  /** after a send: the mentions go */
  readonly reset: () => void;
  /** a paste: task ids become their mentions (true: taken) */
  readonly paste: (e: ClipboardEvent) => boolean;
}

export function usePromptMenu(o: {
  readonly shell: ShellContext;
  readonly where: 'reply' | 'home';
  readonly task?: string;
  /** New task's picked project's folder (its files for @) */
  readonly project?: () => string | undefined;
  readonly text: Accessor<string>;
  readonly setText: (v: string) => void;
  readonly input: () => HTMLTextAreaElement | undefined;
  readonly id: string;
}): PromptMenu {
  const sources = o.shell.promptSources();
  const [at, setAt] = createSignal<MenuAt | null>(null);
  const [active, setActive] = createSignal(0);
  const [mentions, setMentions] = createSignal<readonly Mentioned[]>([]);
  const base = { client: o.shell.client.plugin('shell'), ...(o.task !== undefined && { task: o.task }) };
  // New task's picked project, read when the menu asks (its files for @)
  const ctxNow = (): Parameters<PromptSourceDef['items']>[1] => {
    const project = o.project?.();
    return project ? { ...base, project } : base;
  };
  const items = createMemo((): PromptItem[] => {
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
  const titleOf = (id: string): string | undefined => {
    const peer = (o.shell.client.plugin('shell') as { home: string }).home;
    const key = id.includes(':') ? id : `${peer}:${id}`;
    return o.shell.client.entities.peek<{ title?: string }>('session', key as never)?.title;
  };
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
