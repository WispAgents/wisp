// Markdown: renders the parsed tree with Solid elements, never innerHTML, so the source can hold
// anything (`<script>`, `<img onerror>`) and it shows as text. Blocks that didn't change keep
// their DOM nodes while the source grows (a streaming answer), so nothing flickers or replays.
import { Dynamic } from 'solid-js/web';
import { createMemo, Index, Show, type Accessor, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../../core/props.ts';
import { Code } from '../Code.tsx';
import { Mermaid } from '../Mermaid.tsx';
import type { Block, Inline, ListItem } from './ast.ts';
import { parseMarkdown } from './block.ts';
import '../../css/markdown-blocks.css';

export interface MarkdownProps extends KitProps {
  readonly source: string;
  /** Called on a link click; may preventDefault to open it in-app. */
  readonly onLink?: (href: string, event: MouseEvent) => void;
  /** The heading element `#` maps to (default 3, so an answer's headings sit under the page's). */
  readonly headingBase?: 1 | 2 | 3 | 4;
}

interface Ctx {
  readonly part: PartName;
  readonly base: number;
  readonly onLink: (href: string, event: MouseEvent) => void;
}

function inlines(nodes: readonly Inline[], ctx: Ctx): JSX.Element[] {
  return nodes.map((n): JSX.Element => {
    switch (n.t) {
      case 'text': return n.v;
      case 'code': return <Code part={sub(ctx.part, 'code')}>{n.v}</Code>;
      case 'strong': return <strong data-kit="md-strong">{inlines(n.c, ctx)}</strong>;
      case 'em': return <em data-kit="md-em">{inlines(n.c, ctx)}</em>;
      case 'del': return <del data-kit="md-del">{inlines(n.c, ctx)}</del>;
      case 'br': return <br />;
      case 'link': {
        const local = n.href.startsWith('#');
        return (
          <a
            href={n.href}
            title={n.title ?? undefined}
            target={local ? undefined : '_blank'}
            rel={local ? undefined : 'noopener noreferrer'}
            onClick={(e) => ctx.onLink(n.href, e)}
            data-kit="md-link"
            data-part={sub(ctx.part, 'link')}
          >
            {inlines(n.c, ctx)}
          </a>
        );
      }
      default: {
        const never: never = n;
        return never;
      }
    }
  });
}

function item(it: ListItem, ctx: Ctx, tight: boolean): JSX.Element {
  return (
    <li data-kit="md-li" data-task={it.task === null ? undefined : it.task ? 'done' : 'open'}>
      {it.task === null ? null : <input type="checkbox" checked={it.task} disabled data-kit="md-task" />}
      {blocks(it.c, ctx, tight)}
    </li>
  );
}

function table(b: Extract<Block, { t: 'table' }>, ctx: Ctx): JSX.Element {
  const cell = (tag: 'th' | 'td', c: readonly Inline[], k: number): JSX.Element => (
    <Dynamic component={tag} data-kit={`md-${tag}`} data-align={b.align[k] ?? undefined}>
      {inlines(c, ctx)}
    </Dynamic>
  );
  return (
    <div data-kit="md-table-wrap" data-part={sub(ctx.part, 'table')}>
      <table data-kit="md-table">
        <thead>
          <tr>{b.head.map((c, k) => cell('th', c, k))}</tr>
        </thead>
        <tbody>
          {b.rows.map((row) => (
            <tr>{row.map((c, k) => cell('td', c, k))}</tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** A closed ```mermaid fence (any case): drawn as a diagram. Open, it is still arriving and stays code. */
const isDiagram = (b: Extract<Block, { t: 'code' }>): boolean => !b.open && b.lang.toLowerCase() === 'mermaid';

function block(b: Block, ctx: Ctx, tight: boolean): JSX.Element {
  switch (b.t) {
    case 'heading': {
      const level = Math.min(6, b.level + ctx.base - 1);
      return <Dynamic component={`h${level}`} data-kit="md-heading" data-level={b.level}>{inlines(b.c, ctx)}</Dynamic>;
    }
    // A tight list's paragraphs are bare text in their items, as GFM renders them.
    case 'paragraph': return tight ? <>{inlines(b.c, ctx)}</> : <p data-kit="md-p">{inlines(b.c, ctx)}</p>;
    // A diagram draws once its fence has closed; while it streams it is code, so a half-written graph never errors.
    case 'code':
      return isDiagram(b) ? (
        <Mermaid source={b.v} part={sub(ctx.part, 'mermaid')} />
      ) : (
        <Code block copy lang={b.lang} part={sub(ctx.part, 'code-block')}>{b.v}</Code>
      );
    case 'list': {
      const items = b.items.map((it) => item(it, ctx, b.tight));
      return b.ordered ? (
        <ol data-kit="md-list" start={b.start === 1 ? undefined : b.start} data-tight={b.tight ? '' : undefined}>{items}</ol>
      ) : (
        <ul data-kit="md-list" data-tight={b.tight ? '' : undefined}>{items}</ul>
      );
    }
    case 'quote': return <blockquote data-kit="md-quote">{blocks(b.c, ctx, false)}</blockquote>;
    case 'table': return table(b, ctx);
    case 'hr': return <hr data-kit="md-hr" />;
    // Native <details>: the browser opens and closes it, with no script. One still streaming shows open.
    case 'details':
      return (
        <details data-kit="md-details" data-part={sub(ctx.part, 'details')} open={b.open || !b.closed}>
          {/* without one the browser names it itself ("Details", in the page's language) */}
          {b.summary ? <summary data-kit="md-summary">{inlines(b.summary, ctx)}</summary> : null}
          {blocks(b.c, ctx, false)}
        </details>
      );
    default: {
      const never: never = b;
      return never;
    }
  }
}

function blocks(list: readonly Block[], ctx: Ctx, tight: boolean): JSX.Element[] {
  return list.map((b) => block(b, ctx, tight));
}

function sameInlines(a: readonly Inline[], b: readonly Inline[]): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!sameInline(a[i]!, b[i]!)) return false;
  return true;
}

function sameInline(a: Inline, b: Inline): boolean {
  if (a === b) return true;
  if (a.t !== b.t) return false;
  switch (a.t) {
    case 'text':
    case 'code':
      return a.v === (b as typeof a).v;
    case 'strong':
    case 'em':
    case 'del':
      return sameInlines(a.c, (b as typeof a).c);
    case 'link': {
      const o = b as typeof a;
      return a.href === o.href && a.title === o.title && sameInlines(a.c, o.c);
    }
    case 'br':
      return true;
  }
}

function sameBlocks(a: readonly Block[], b: readonly Block[]): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (!sameBlock(a[i]!, b[i]!)) return false;
  return true;
}

/** Structural AST equality without JSON.stringify allocations, so streaming keeps unchanged blocks in place fast. */
export function sameBlock(a: Block, b: Block): boolean {
  if (a === b) return true;
  if (a.t !== b.t) return false;
  switch (a.t) {
    case 'hr':
      return true;
    case 'heading': {
      const o = b as typeof a;
      return a.level === o.level && sameInlines(a.c, o.c);
    }
    case 'paragraph':
      return sameInlines(a.c, (b as typeof a).c);
    case 'code': {
      const o = b as typeof a;
      return a.lang === o.lang && a.v === o.v && a.open === o.open;
    }
    case 'details': {
      const o = b as typeof a;
      if (a.open !== o.open || a.closed !== o.closed || !sameBlocks(a.c, o.c)) return false;
      return a.summary === null || o.summary === null ? a.summary === o.summary : sameInlines(a.summary, o.summary);
    }
    case 'quote':
      return sameBlocks(a.c, (b as typeof a).c);
    case 'list': {
      const o = b as typeof a;
      if (a.ordered !== o.ordered || a.start !== o.start || a.tight !== o.tight || a.items.length !== o.items.length) return false;
      for (let i = 0; i < a.items.length; i++) {
        const x = a.items[i]!;
        const y = o.items[i]!;
        if (x.task !== y.task || !sameBlocks(x.c, y.c)) return false;
      }
      return true;
    }
    case 'table': {
      const o = b as typeof a;
      if (a.align.length !== o.align.length || a.head.length !== o.head.length || a.rows.length !== o.rows.length) return false;
      for (let i = 0; i < a.align.length; i++) if (a.align[i] !== o.align[i]) return false;
      for (let i = 0; i < a.head.length; i++) if (!sameInlines(a.head[i]!, o.head[i]!)) return false;
      for (let r = 0; r < a.rows.length; r++) {
        const ra = a.rows[r]!;
        const rb = o.rows[r]!;
        if (ra.length !== rb.length) return false;
        for (let c = 0; c < ra.length; c++) if (!sameInlines(ra[c]!, rb[c]!)) return false;
      }
      return true;
    }
  }
}

function TopBlock(props: { readonly block: Accessor<Block>; readonly ctx: Ctx }): JSX.Element {
  const stable = createMemo(() => props.block(), undefined, { equals: sameBlock });
  // A diagram keeps its component while its source changes (an edited or re-streamed fence), so it shows the last
  // drawing until the next one is ready instead of a fresh skeleton, and its own debounce sees every change.
  const diagram = createMemo(() => {
    const b = stable();
    return b.t === 'code' && isDiagram(b) ? b.v : undefined;
  });
  return (
    <Show when={diagram() !== undefined} fallback={<>{block(stable(), props.ctx, false)}</>}>
      <Mermaid source={diagram() ?? ''} part={sub(props.ctx.part, 'mermaid')} />
    </Show>
  );
}

export function Markdown(props: MarkdownProps): JSX.Element {
  const tree = createMemo(() => parseMarkdown(props.source));
  const ctx: Ctx = {
    get part() {
      return props.part ?? 'ui.markdown';
    },
    get base() {
      return props.headingBase ?? 3;
    },
    onLink: (href, e) => props.onLink?.(href, e),
  };
  return (
    <div id={props.id} data-kit="markdown" data-part={props.part ?? 'ui.markdown'}>
      <Index each={tree()}>{(b) => <TopBlock block={b} ctx={ctx} />}</Index>
    </div>
  );
}
