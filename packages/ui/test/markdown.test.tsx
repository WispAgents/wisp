import { afterEach, describe, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import { createSignal } from 'solid-js';
import { Markdown, parseInline, parseMarkdown, safeHref, type MarkdownBlock as Block, type MarkdownInline as Inline } from '../src/content/rich.ts';
import { sameBlock } from '../src/content/markdown/Markdown.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

function mount(ui: () => import('solid-js').JSX.Element): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(ui, host);
  return host;
}

const t = (v: string): Inline => ({ t: 'text', v });

describe('inline', () => {
  test.each<[string, Inline[]]>([
    ['plain', [t('plain')]],
    ['a **b** c', [t('a '), { t: 'strong', c: [t('b')] }, t(' c')]],
    ['*em* and _em_', [{ t: 'em', c: [t('em')] }, t(' and '), { t: 'em', c: [t('em')] }]],
    ['***both***', [{ t: 'strong', c: [{ t: 'em', c: [t('both')] }] }]],
    ['~~gone~~', [{ t: 'del', c: [t('gone')] }]],
    ['`a **b**`', [{ t: 'code', v: 'a **b**' }]],
    ['`` a`b ``', [{ t: 'code', v: 'a`b' }]],
    ['snake_case_name', [t('snake_case_name')]],
    ['2 * 3 * 4', [t('2 * 3 * 4')]],
    ['\\*not em\\*', [t('*not em*')]],
    ['[wisp](https://trywisp.dev "Home")', [{ t: 'link', href: 'https://trywisp.dev', title: 'Home', c: [t('wisp')] }]],
    ['<https://a.dev/x>', [{ t: 'link', href: 'https://a.dev/x', title: null, c: [t('https://a.dev/x')] }]],
    ['see https://a.dev/x.', [t('see '), { t: 'link', href: 'https://a.dev/x', title: null, c: [t('https://a.dev/x')] }, t('.')]],
    ['(https://a.dev/(x))', [t('('), { t: 'link', href: 'https://a.dev/(x)', title: null, c: [t('https://a.dev/(x)')] }, t(')')]],
    ['line  \nnext', [t('line'), { t: 'br' }, t('next')]],
    ['line\nnext', [t('line\nnext')]],
    ['unclosed `tick', [t('unclosed `tick')]],
    ['**unclosed', [t('**unclosed')]],
  ])('%j', (src, want) => {
    expect(parseInline(src)).toEqual(want);
  });

  test('a link inside link text stays text', () => {
    expect(parseInline('[a [b](x) c](y)')).toEqual([{ t: 'link', href: 'y', title: null, c: [t('a [b](x) c')] }]);
  });

  test('unsafe link schemes become plain text', () => {
    expect(parseInline('[click](javascript:alert(1))')).toEqual([t('click')]);
    expect(parseInline('[x](JAVA\nSCRIPT:alert(1))')).toEqual([t('[x](JAVA\nSCRIPT:alert(1))')]);
    expect(parseInline('<javascript:alert(1)>')).toEqual([t('<javascript:alert(1)>')]);
  });
});

describe('safeHref', () => {
  test.each([
    ['https://a.dev', 'https://a.dev'],
    ['mailto:a@b.c', 'mailto:a@b.c'],
    ['/docs', '/docs'],
    ['#top', '#top'],
    ['./x.md', './x.md'],
    ['javascript:alert(1)', undefined],
    [' JaVaScRiPt:alert(1)', undefined],
    ['java\tscript:alert(1)', undefined],
    ['data:text/html,<b>', undefined],
    ['vbscript:x', undefined],
    ['file:///etc/passwd', undefined],
  ])('%s', (url, want) => {
    expect(safeHref(url)).toBe(want);
  });
});

const kinds = (blocks: readonly Block[]): string[] => blocks.map((b) => b.t);

describe('blocks', () => {
  test('headings, paragraphs, rules and quotes', () => {
    const b = parseMarkdown('# Title #\n\nOne\ntwo\n\n---\n\n> quoted\n> > nested\n\n#nothashtag');
    expect(kinds(b)).toEqual(['heading', 'paragraph', 'hr', 'quote', 'paragraph']);
    expect(b[0]).toEqual({ t: 'heading', level: 1, c: [t('Title')] });
    const q = b[3];
    expect(q?.t === 'quote' && kinds(q.c)).toEqual(['paragraph', 'quote']);
  });

  test('fences keep their content verbatim, and an unclosed one runs to the end', () => {
    expect(parseMarkdown('```ts\nconst a = 1;\n\n# not a heading\n```')).toEqual([{ t: 'code', lang: 'ts', v: 'const a = 1;\n\n# not a heading', open: false }]);
    expect(parseMarkdown('~~~\nstill\nstreaming')).toEqual([{ t: 'code', lang: '', v: 'still\nstreaming', open: true }]);
  });

  test('indented code', () => {
    expect(parseMarkdown('    npm test\n    npm run x\n\nafter')).toEqual([{ t: 'code', lang: '', v: 'npm test\nnpm run x', open: false }, { t: 'paragraph', c: [t('after')] }]);
  });

  test('nested lists by indentation', () => {
    const [list] = parseMarkdown('- a\n  - b\n    - c\n- d');
    expect(list?.t).toBe('list');
    if (list?.t !== 'list') return;
    expect(list.items.length).toBe(2);
    expect(list.tight).toBe(true);
    const inner = list.items[0]?.c[1];
    expect(inner?.t === 'list' && inner.items.length).toBe(1);
    const deepest = inner?.t === 'list' ? inner.items[0]?.c[1] : undefined;
    expect(deepest?.t).toBe('list');
  });

  test('a fence inside a list item stays in the item', () => {
    const [list, ...rest] = parseMarkdown('1. Install\n   ```sh\n   npm i\n   - not an item\n   ```\n2. Run');
    expect(rest).toEqual([]);
    if (list?.t !== 'list') throw new Error('no list');
    expect(list.ordered).toBe(true);
    expect(list.items.length).toBe(2);
    expect(list.items[0]?.c[1]).toEqual({ t: 'code', lang: 'sh', v: 'npm i\n- not an item', open: false });
  });

  test('ordered lists keep their start; a blank line between items makes it loose', () => {
    const [list] = parseMarkdown('3. three\n\n4. four');
    expect(list).toMatchObject({ t: 'list', ordered: true, start: 3, tight: false });
  });

  test('task lists', () => {
    const [list] = parseMarkdown('- [x] done\n- [ ] open\n- plain');
    if (list?.t !== 'list') throw new Error('no list');
    expect(list.items.map((i) => i.task)).toEqual([true, false, null]);
  });

  test('tables with alignment, escaped pipes and pipes in code spans', () => {
    const [table] = parseMarkdown('| Name | Size | Note |\n|:--|--:|:-:|\n| `a|b` | 12 | x \\| y |\n| short |');
    if (table?.t !== 'table') throw new Error('no table');
    expect(table.align).toEqual(['left', 'right', 'center']);
    expect(table.rows[0]?.[0]).toEqual([{ t: 'code', v: 'a|b' }]);
    expect(table.rows[0]?.[2]).toEqual([t('x | y')]);
    // A short row is padded to the header's width.
    expect(table.rows[1]?.length).toBe(3);
  });

  test('a paragraph ends at a list or heading but not at "2024."', () => {
    expect(kinds(parseMarkdown('Intro\n- item'))).toEqual(['paragraph', 'list']);
    expect(kinds(parseMarkdown('Intro\n## Head'))).toEqual(['paragraph', 'heading']);
    expect(kinds(parseMarkdown('In\n2024. was long'))).toEqual(['paragraph']);
  });

  test('Windows line ends and tabs', () => {
    expect(parseMarkdown('# A\r\n\r\n-\tb')).toEqual([
      { t: 'heading', level: 1, c: [t('A')] },
      { t: 'list', ordered: false, start: 1, tight: true, items: [{ task: null, c: [{ t: 'paragraph', c: [t('b')] }] }] },
    ]);
  });
});

describe('<Markdown>', () => {
  test('renders its data-part, and its children name theirs', () => {
    const host = mount(() => <Markdown source={'Hi `x` [a](https://a.dev)\n\n```\ncode\n```\n\n| a |\n|---|\n| b |'} part="answer.body" />);
    expect(host.querySelector('[data-part="answer.body"]')).not.toBeNull();
    expect(host.querySelector('[data-part="answer.body.code"]')?.textContent).toBe('x');
    expect(host.querySelector('[data-part="answer.body.link"]')).not.toBeNull();
    expect(host.querySelector('[data-part="answer.body.code-block"]')).not.toBeNull();
    expect(host.querySelector('[data-part="answer.body.table"]')).not.toBeNull();
    expect(mount(() => <Markdown source="x" />).querySelector('[data-part="ui.markdown"]')).not.toBeNull();
  });

  test('escapes HTML: no element comes from the source, the text shows', () => {
    const src = '<img src=x onerror=alert(1)><script>alert(2)</script>\n\n**<b>bold</b>**\n\n<iframe src="https://x"></iframe>';
    const host = mount(() => <Markdown source={src} />);
    for (const tag of ['img', 'script', 'b', 'iframe']) expect(host.querySelector(tag)).toBeNull();
    expect(host.textContent).toContain('<img src=x onerror=alert(1)><script>alert(2)</script>');
    expect(host.textContent).toContain('<b>bold</b>');
    expect(host.querySelectorAll('[onerror]').length).toBe(0);
  });

  test('javascript: links render no anchor', () => {
    const host = mount(() => <Markdown source={'[click](javascript:alert(1)) and <javascript:x>'} />);
    expect(host.querySelector('a')).toBeNull();
    expect(host.textContent).toContain('click');
  });

  test('links open in a new tab without the opener, and report clicks', () => {
    const onLink = vi.fn((_href: string, e: MouseEvent) => e.preventDefault());
    const host = mount(() => <Markdown source="[docs](https://trywisp.dev) [top](#top)" onLink={onLink} />);
    const [ext, local] = [...host.querySelectorAll('a')];
    expect(ext?.getAttribute('target')).toBe('_blank');
    expect(ext?.getAttribute('rel')).toBe('noopener noreferrer');
    expect(local?.getAttribute('target')).toBeNull();
    ext?.click();
    expect(onLink).toHaveBeenCalledWith('https://trywisp.dev', expect.anything());
  });

  test('headings start at h3 by default, and headingBase moves them', () => {
    expect(mount(() => <Markdown source="# A" />).querySelector('h3')?.textContent).toBe('A');
    expect(mount(() => <Markdown source="## B" headingBase={1} />).querySelector('h2')?.textContent).toBe('B');
  });

  test('task items render disabled checkboxes; tight items have no <p>', () => {
    const host = mount(() => <Markdown source={'- [x] done\n- [ ] open'} />);
    const boxes = [...host.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')];
    expect(boxes.map((b) => b.checked)).toEqual([true, false]);
    expect(boxes.every((b) => b.disabled)).toBe(true);
    expect(host.querySelector('li p')).toBeNull();
  });

  test('a long unbroken word is kept whole in the text (CSS breaks it)', () => {
    const word = 'a'.repeat(500);
    expect(mount(() => <Markdown source={word} />).textContent).toBe(word);
  });

  test('growing source keeps the blocks that did not change (streaming)', () => {
    const [src, setSrc] = createSignal('First paragraph.\n\nSecond');
    const host = mount(() => <Markdown source={src()} />);
    const first = host.querySelector('p');
    setSrc('First paragraph.\n\nSecond, longer now');
    expect(host.querySelector('p')).toBe(first);
    expect(host.querySelectorAll('p')[1]?.textContent).toBe('Second, longer now');
  });

  test('sameBlock compares headings, lists, quotes, code and tables structurally without JSON.stringify', () => {
    const src = '# H1\n\n- [x] a\n- [ ] b\n\n> q\n\n```ts\nconst x = 1;\n```\n\n| A | B |\n|:--|:--|\n| 1 | `2` |\n\n---';
    const a = parseMarkdown(src);
    const b = parseMarkdown(src);
    expect(a.length).toBe(b.length);
    for (let i = 0; i < a.length; i++) expect(sameBlock(a[i]!, b[i]!)).toBe(true);
    const c = parseMarkdown(src.replace('const x = 1;', 'const x = 2;').replace('| 1 |', '| 9 |'));
    expect(sameBlock(a[3]!, c[3]!)).toBe(false);
    expect(sameBlock(a[4]!, c[4]!)).toBe(false);
    expect(sameBlock(a[0]!, a[1]!)).toBe(false);
  });
});

describe('fences that are still open', () => {
  test('a fence is open until its closer arrives; indented code never is', () => {
    expect(parseMarkdown('```mermaid\ngraph TD')).toEqual([{ t: 'code', lang: 'mermaid', v: 'graph TD', open: true }]);
    expect(parseMarkdown('```mermaid\ngraph TD\n```')).toEqual([{ t: 'code', lang: 'mermaid', v: 'graph TD', open: false }]);
    expect(parseMarkdown('    code')).toEqual([{ t: 'code', lang: '', v: 'code', open: false }]);
  });

  test('sameBlock tells an open fence from the closed one with the same text', () => {
    const [a] = parseMarkdown('```mermaid\ngraph TD');
    const [b] = parseMarkdown('```mermaid\ngraph TD\n```');
    expect(sameBlock(a!, b!)).toBe(false);
  });
});

describe('setext headings', () => {
  test('=== under text is an h1, --- an h2 (and no rule)', () => {
    expect(parseMarkdown('Title\n=====')).toEqual([{ t: 'heading', level: 1, c: [t('Title')] }]);
    expect(parseMarkdown('Sub\n---')).toEqual([{ t: 'heading', level: 2, c: [t('Sub')] }]);
    expect(parseMarkdown('Two\nlines\n  ==  ')).toEqual([{ t: 'heading', level: 1, c: [t('Two\nlines')] }]);
  });

  test('a rule after a blank line, a list or a fence is still a rule', () => {
    expect(kinds(parseMarkdown('Para\n\n---'))).toEqual(['paragraph', 'hr']);
    expect(kinds(parseMarkdown('- a\n---'))).toEqual(['list', 'hr']);
    expect(kinds(parseMarkdown('```\nx\n```\n---'))).toEqual(['code', 'hr']);
    expect(kinds(parseMarkdown('# H\n---'))).toEqual(['heading', 'hr']);
  });

  test('a list streaming in right under a prose line never flashes the line as an h2', () => {
    // Every prefix of 'Steps:\n- one' keeps 'Steps:' a paragraph: '-' as the last line and '- ' are list openers.
    expect(kinds(parseMarkdown('Steps:\n-'))).toEqual(['paragraph', 'list']);
    expect(kinds(parseMarkdown('Steps:\n- '))).toEqual(['paragraph', 'list']);
    expect(kinds(parseMarkdown('Para\n- '))).toEqual(['paragraph', 'list']);
    expect(kinds(parseMarkdown('Steps:\n-\n'))).toEqual(['paragraph', 'list']);
    expect(kinds(parseMarkdown('Steps:\n- one'))).toEqual(['paragraph', 'list']);
    const src = 'Steps:\n- one\n- two';
    for (let n = 'Steps:\n'.length; n <= src.length; n++) expect(kinds(parseMarkdown(src.slice(0, n)))[0]).toBe('paragraph');
  });

  test('a lone - underline with text after it is still an h2, and -- is never a list', () => {
    expect(kinds(parseMarkdown('Sub\n-\nmore'))).toEqual(['heading', 'paragraph']);
    expect(parseMarkdown('Sub\n--')).toEqual([{ t: 'heading', level: 2, c: [t('Sub')] }]);
  });

  test('renders as a heading element', () => {
    const host = mount(() => <Markdown source={'Sub\n---'} headingBase={1} />);
    expect(host.querySelector('h2')?.textContent).toBe('Sub');
    expect(host.querySelector('hr')).toBeNull();
  });
});

describe('<details>', () => {
  const example = '<details>\n<summary>More</summary>\n\nSome **text**\n</details>';

  test('parses its summary and its body as Markdown, counting nested ones', () => {
    const [d] = parseMarkdown(example);
    expect(d).toEqual({ t: 'details', open: false, summary: [t('More')], c: [{ t: 'paragraph', c: [t('Some '), { t: 'strong', c: [t('text')] }] }], closed: true });
    const nested = parseMarkdown('<details open>\n<details>\ninner\n</details>\nouter\n</details>\nafter');
    expect(kinds(nested)).toEqual(['details', 'paragraph']);
    const outer = nested[0];
    if (outer?.t !== 'details') throw new Error('no details');
    expect(outer.open).toBe(true);
    expect(kinds(outer.c)).toEqual(['details', 'paragraph']);
  });

  test('renders a native details element with its summary and body', () => {
    const host = mount(() => <Markdown source={example} />);
    const d = host.querySelector('details');
    expect(d?.getAttribute('data-kit')).toBe('md-details');
    expect(d?.querySelector('summary')?.textContent).toBe('More');
    expect(d?.querySelector('strong')?.textContent).toBe('text');
    expect(d?.open).toBe(false);
  });

  test('one still streaming (no closer) renders open', () => {
    const src = '<details>\n<summary>More</summary>\n\nstill coming';
    expect(parseMarkdown(src)[0]).toMatchObject({ t: 'details', closed: false });
    expect(mount(() => <Markdown source={src} />).querySelector('details')?.open).toBe(true);
  });

  test('inline in a sentence it stays text, and other HTML stays text', () => {
    const host = mount(() => <Markdown source={'Use <details> here.\n\n<script>alert(1)</script>\n\n<details onclick=x>'} />);
    expect(host.querySelector('details')).toBeNull();
    expect(host.querySelector('script')).toBeNull();
    expect(host.textContent).toContain('Use <details> here.');
    expect(host.textContent).toContain('<script>alert(1)</script>');
  });

  test('sameBlock compares details structurally', () => {
    const [a] = parseMarkdown(example);
    const [b] = parseMarkdown(example);
    const [c] = parseMarkdown(example.replace('More', 'Less'));
    expect(sameBlock(a!, b!)).toBe(true);
    expect(sameBlock(a!, c!)).toBe(false);
  });
});
