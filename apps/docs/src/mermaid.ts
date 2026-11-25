// The docs site's diagrams (ADR-0118): a small island, loaded on every page from Head.astro, that finds the page's
// ```mermaid fences (Expressive Code's `pre[data-language="mermaid"]`) and only then imports the bundled mermaid
// package, never a CDN: a page without a diagram loads nothing more. Strict security level, the site's light or dark
// theme, drawn again when the reader switches it. Until it draws, and if it can't, the fence stays the code block it
// was (with its copy button). Astro 7's Markdown processor runs no remark plugins without another dependency, so the
// fences are found in the page rather than rewritten at build time.

type Api = {
  initialize: (config: Record<string, unknown>) => void;
  render: (id: string, source: string) => Promise<{ svg: string }>;
};

interface Diagram {
  readonly block: HTMLElement;
  readonly source: string;
  readonly view: HTMLElement;
}

const found: Diagram[] = [];
for (const pre of document.querySelectorAll<HTMLElement>('pre[data-language="mermaid"]')) {
  const block = pre.closest<HTMLElement>('.expressive-code') ?? pre;
  // one line of code per .ec-line: their text, joined, is the fence's source
  const lines = [...pre.querySelectorAll('.ec-line')].map((l) => l.textContent ?? '');
  const source = lines.length > 0 ? lines.join('\n') : (pre.textContent ?? '');
  const view = document.createElement('div');
  view.className = 'wisp-mermaid not-content';
  view.setAttribute('role', 'img');
  view.setAttribute('aria-label', 'Diagram');
  view.hidden = true;
  block.after(view);
  found.push({ block, source, view });
}

if (found.length > 0) {
  let seq = 0;
  const dark = (): boolean => {
    const t = document.documentElement.dataset['theme'];
    return t === 'dark' || (t !== 'light' && matchMedia('(prefers-color-scheme: dark)').matches);
  };
  const draw = async (api: Api): Promise<void> => {
    const d = dark();
    api.initialize({ startOnLoad: false, securityLevel: 'strict', theme: d ? 'dark' : 'default', darkMode: d, suppressErrorRendering: true, htmlLabels: false, flowchart: { htmlLabels: false } });
    for (const f of found) {
      try {
        const { svg } = await api.render(`wisp-docs-mermaid-${++seq}`, f.source);
        // mermaid's own output at securityLevel 'strict', sanitised by it; the page's source never becomes HTML
        f.view.innerHTML = svg;
        f.view.hidden = false;
        f.block.hidden = true;
      } catch {
        f.view.hidden = true;
        f.block.hidden = false;
      }
    }
  };
  void import('mermaid').then((m) => {
    const api = m.default as unknown as Api;
    void draw(api);
    new MutationObserver(() => void draw(api)).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  });
}
