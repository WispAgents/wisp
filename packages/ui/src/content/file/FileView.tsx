// FileView: a file to read and edit (v1 features/editor.js, design/v1-reference/task.editor): line numbers, the
// highlighted text with an editable layer over it (a textarea whose text is transparent, so the colours show and
// the caret, selection and typing are the browser's own), and an optional why-gutter: each line's task, a click
// opens it. The gutter and the numbers scroll with the text; nothing reflows on a keystroke but the text itself.
import { For, Index, Show, createEffect, createMemo, createSelector, createSignal, on, onCleanup, onMount, untrack, type Accessor, type JSX } from 'solid-js';
import { flag, sub, type KitProps, type PartName } from '../../core/props.ts';
import { highlight, type Token } from '../highlight.ts';
import { hoverWords, lineAt, nameAt, nameCellAt } from './file-text.ts';
import { useTimeout } from '../../blocks/rich-scope.ts';
import '../../css/file-view.css';

const CHUNK = 100;

function countLines(s: string): number {
  let n = 1;
  for (let i = 0; i < s.length; i++) if (s.charCodeAt(i) === 10) n++;
  return n;
}

function offsetOfLine(s: string, line: number): number {
  if (line <= 1) return 0;
  let seen = 1;
  for (let i = 0; i < s.length; i++) {
    if (s.charCodeAt(i) === 10 && ++seen === line) return i + 1;
  }
  return s.length;
}

function sameLine(a: readonly Token[], b: readonly Token[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i]?.kind !== b[i]?.kind || a[i]?.text !== b[i]?.text) return false;
  }
  return true;
}

/** Tokenize per line, reusing unchanged line arrays from `prev` so typing only updates the edited line's DOM. */
function tokenLines(code: string, lang: string | undefined, prev?: readonly (readonly Token[])[]): (readonly Token[])[] {
  const out: Token[][] = [[]];
  for (const tk of highlight(code, lang)) {
    const parts = tk.text.split('\n');
    for (let i = 0; i < parts.length; i++) {
      if (i > 0) out.push([]);
      const text = parts[i];
      if (text) out[out.length - 1]?.push({ kind: tk.kind, text });
    }
  }
  if (!prev) return out;
  return out.map((line, i) => {
    const old = prev[i];
    return old && sameLine(old, line) ? old : line;
  });
}

function LineTokens(props: { readonly line: Accessor<readonly Token[]> }): JSX.Element {
  const stable = createMemo(() => props.line());
  return (
    <div data-kit="fv-line">
      <For each={stable()}>{(tk) => (tk.kind ? <span data-tk={tk.kind}>{tk.text}</span> : tk.text)}</For>
    </div>
  );
}

/** One line's mark in the why-gutter: who wrote it (a short label) and why (its title). */
export interface GutterMark {
  readonly label: string;
  readonly title?: string;
  /** a new task starts here (the line before was another's): the label shows, else only a thread */
  readonly first?: boolean;
  readonly onOpen?: () => void;
}

export interface FileViewProps extends KitProps {
  readonly text: string;
  /** a language for colours (`ts`, `py`…), from the file's extension */
  readonly lang?: string | undefined;
  /** typing changes it; without it the view is read-only */
  readonly onInput?: (text: string) => void;
  /** ⌘S / Ctrl+S */
  readonly onSave?: () => void;
  /** the line to show (scrolled to, its number marked, the caret at its start): Go to definition's */
  readonly line?: number | undefined;
  /** the lines selected (1-based, inclusive), as the selection changes; undefined when it's only a caret */
  readonly onSelectLines?: (lines: { readonly from: number; readonly to: number } | undefined) => void;
  /** ⌘-click (Ctrl-click) or F12 on a name: where is it declared? */
  readonly onDefinition?: (name: string) => void;
  /** the pointer rests on a name (600 ms): what it is, from the task's language server (v1 editor.js hoverOn); undefined
   *  or '' shows nothing. Not on touch screens. */
  readonly onHover?: (at: { readonly line: number; readonly col: number }) => Promise<string | undefined>;
  /** the why-gutter, one mark per line (1-based: index 0 is line 1); none: no gutter */
  readonly gutter?: readonly (GutterMark | undefined)[] | undefined;
  readonly 'aria-label': string;
  /** takes the keyboard focus as it opens (v1 editor.js: the text, not on a phone), so what's typed next goes into the
   *  file and never runs a list shortcut (audit E #1: after Open file… the focus was left on the page and E marked the
   *  task done). Additive (2026-11-19). */
  readonly autofocus?: boolean;
}

export function FileView(props: FileViewProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.file-view';
  const lines = createMemo(() => countLines(props.text));
  const byLine = createMemo<(readonly Token[])[]>((prev) => tokenLines(props.text, props.lang, prev));
  const chunks = createMemo(() => {
    const all = byLine();
    const out: (readonly (readonly Token[])[])[] = [];
    for (let i = 0; i < all.length; i += CHUNK) out.push(all.slice(i, i + CHUNK));
    return out;
  });
  const numbers = createMemo(() => Array.from({ length: lines() }, (_, i) => i + 1));
  const isActiveLine = createSelector(() => props.line);
  let box: HTMLDivElement | undefined;
  let area: HTMLTextAreaElement | undefined;
  const selected = (): void => {
    if (!area || !props.onSelectLines) return;
    const { selectionStart: a, selectionEnd: b, value } = area;
    props.onSelectLines(a === b ? undefined : { from: lineAt(value, a), to: lineAt(value, Math.max(a, b - 1)) });
  };
  const definition = (): void => {
    if (!area || !props.onDefinition) return;
    const name = nameAt(area.value, area.selectionStart);
    if (name) props.onDefinition(name);
  };
  // Go to definition's line: track only `props.line` so editing text never jumps the caret back or forces layout
  createEffect(
    on(
      () => props.line,
      (n) => {
        if (!n || !area || !box) return;
        const at = offsetOfLine(untrack(() => props.text), n);
        area.setSelectionRange(at, at);
        const row = box.querySelector<HTMLElement>(`[data-kit="fv-nums"] > [data-line="${n}"]`);
        if (row) box.scrollTop = Math.max(0, row.offsetTop - box.clientHeight / 3);
      },
    ),
  );
  onMount(() => {
    if (!props.autofocus || !area) return;
    // the caret at the top (a new value leaves it after the last line, v1 editor.js), unless a line is asked for
    if (!props.line) area.setSelectionRange(0, 0);
    area.focus({ preventScroll: true });
  });
  const hover = useHover(props, () => area);
  const key = (e: KeyboardEvent): void => {
    hover.hide();
    if (e.key === 'F12') {
      e.preventDefault();
      definition();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      props.onSave?.();
      return;
    }
    // Tab indents (two spaces) instead of leaving the editor; Shift+Tab leaves it
    if (e.key === 'Tab' && !e.shiftKey && props.onInput) {
      e.preventDefault();
      const el = e.currentTarget as HTMLTextAreaElement;
      el.setRangeText('  ', el.selectionStart, el.selectionEnd, 'end');
      props.onInput(el.value);
    }
  };
  return (
    <div data-kit="fv-wrap">
      <div ref={(el) => (box = el)} id={props.id} data-kit="file-view" data-part={part()} data-gutter={flag(props.gutter !== undefined)} style={{ '--k-digits': String(String(lines()).length) }}>
        <Show when={props.gutter}>
          {(g) => (
            <div data-kit="fv-why" data-part={sub(part(), 'why')} aria-hidden="false">
              <For each={numbers()}>
                {(n) => {
                  const m = (): GutterMark | undefined => g()[n - 1];
                  return (
                    <Show when={m()} fallback={<span data-kit="fv-why-line" />}>
                      {(mark) => (
                        <button
                          type="button"
                          data-kit="fv-why-line"
                          data-first={flag(mark().first)}
                          title={mark().title ?? mark().label}
                          tabIndex={mark().first ? 0 : -1}
                          onClick={() => mark().onOpen?.()}
                        >
                          <Show when={mark().first}>{mark().label}</Show>
                        </button>
                      )}
                    </Show>
                  );
                }}
              </For>
            </div>
          )}
        </Show>
        <div data-kit="fv-nums" data-part={sub(part(), 'numbers')} aria-hidden="true">
          <For each={numbers()}>{(n) => <span data-line={n} data-on={flag(isActiveLine(n))}>{n}</span>}</For>
        </div>
        <div data-kit="fv-text">
          <pre data-kit="fv-pre" aria-hidden="true">
            <code data-lang={props.lang}>
              <Index each={chunks()}>
                {(chunk) => (
                  <div data-kit="fv-chunk" data-lazy={flag(lines() > CHUNK * 2)} style={{ '--k-lines': String(chunk().length) }}>
                    <Index each={chunk()}>{(line) => <LineTokens line={line} />}</Index>
                  </div>
                )}
              </Index>
            </code>
          </pre>
          <textarea
            ref={(el) => (area = el)}
            data-kit="fv-input"
            data-part={sub(part(), 'input')}
            aria-label={props['aria-label']}
            spellcheck={false}
            autocapitalize="off"
            autocomplete="off"
            wrap="off"
            readOnly={!props.onInput}
            value={props.text}
            onInput={(e) => props.onInput?.(e.currentTarget.value)}
            onKeyDown={key}
            onSelect={selected}
            onKeyUp={selected}
            onMouseUp={selected}
            onClick={(e) => (e.metaKey || e.ctrlKey) && definition()}
            onMouseMove={hover.move}
            onMouseLeave={hover.hide}
          />
          <Show when={hover.tip()}>
            {(h) => (
              <div data-kit="fv-hover" data-part={sub(part(), 'hover')} data-below={flag(h().below)} role="tooltip" style={{ '--k-x': `${h().x}px`, '--k-y': `${h().y}px` }}>
                <pre>{h().text}</pre>
              </div>
            )}
          </Show>
        </div>
      </div>
    </div>
  );
}

interface Tip {
  readonly text: string;
  /** px inside the text box: the pointer's x, the top of the name's line (under it when there's no room above) */
  readonly x: number;
  readonly y: number;
  readonly below: boolean;
}

/** v1 editor.js hoverOn: rest the pointer on a name and the language server says what it is, above its line. A move,
 *  a key or leaving hides it; an answer that comes after the pointer moved on is dropped. */
function useHover(props: FileViewProps, area: () => HTMLTextAreaElement | undefined): { tip: () => Tip | undefined; move: (e: MouseEvent) => void; hide: () => void } {
  const [tip, setTip] = createSignal<Tip | undefined>(undefined);
  const wait = useTimeout();
  let asked = 0;
  let charWidth = 0;
  const hide = (): void => {
    wait.stop();
    asked++;
    setTip(undefined);
  };
  onCleanup(hide);
  const move = (e: MouseEvent): void => {
    hide();
    const el = area();
    const ask = props.onHover;
    if (!el || !ask || matchMedia('(hover: none)').matches) return;
    const n = asked;
    wait.start(600, () => {
      const cs = getComputedStyle(el);
      if (!charWidth) {
        const c = document.createElement('canvas').getContext('2d');
        if (c) c.font = `${cs.fontSize} ${cs.fontFamily}`;
        charWidth = c ? c.measureText('M'.repeat(40)).width / 40 : 0;
      }
      const r = el.getBoundingClientRect();
      const m = { padTop: parseFloat(cs.paddingTop) || 0, padLeft: parseFloat(cs.paddingLeft) || 0, lineHeight: parseFloat(cs.lineHeight) || 20, charWidth };
      const at = nameCellAt(el.value, e.clientX - r.left, e.clientY - r.top, m);
      if (!at) return;
      const top = m.padTop + (at.line - 1) * m.lineHeight;
      void ask(at).then(
        (raw) => {
          const text = hoverWords(raw);
          // above the name's line, never over it; under it only near the top
          if (n === asked && text) setTip({ text, x: e.clientX - r.left, y: top < 48 ? top + m.lineHeight : top, below: top < 48 });
        },
        () => undefined,
      );
    });
  };
  return { tip, move, hide };
}
