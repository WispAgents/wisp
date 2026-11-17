// My Stuff's card (v1 stuff.js paintCard): a click opens it, its tools don't; a pending card does nothing; an app has
// Launch on its preview; a doc shows its opening as a page; the kinds and groups say their counts.
import { describe, expect, it } from 'vitest';
import { StuffCard, StuffGroup, StuffKind, StuffNone } from '../src/app/StuffCard.tsx';
import { MediaViewer } from '../src/blocks/Viewer.tsx';
import { mount } from './helpers.tsx';

const base = { id: 'a1', kind: 'doc', icon: 'file' as const, title: 'release notes', label: 'Doc: release notes', facts: 'Doc · MD · 151 B', time: { label: '22:31', title: 'Tue 17 Nov 2026, 22:31' } };

describe('StuffCard', () => {
  it('opens on a click, not on its tools; the tools say what they do', () => {
    const did: string[] = [];
    const host = mount(() => (
      <StuffCard {...base} thumb={{ kind: 'doc', markdown: '# Release notes\n\nThe **v0.58** release' }} copy={{ label: 'Copy', onClick: () => did.push('copy') }} more={{ label: 'More', onClick: () => did.push('more') }} star={{ label: 'Star', onClick: () => did.push('star') }} onOpen={() => did.push('open')} />
    ));
    const card = host.querySelector<HTMLElement>("[data-kit='st-card']");
    card?.click();
    host.querySelector<HTMLButtonElement>("[data-kit='st-more']")?.click();
    host.querySelector<HTMLButtonElement>("[data-kit='st-star']")?.click();
    expect(did).toEqual(['open', 'more', 'star']);
    expect(host.querySelector("[data-kit='st-paper'] [data-kit='markdown']")?.textContent).toContain('Release notes');
    expect(host.querySelector("[data-kit='st-sub'] time")?.textContent).toBe('22:31');
    expect(card?.getAttribute('aria-label')).toBe('Doc: release notes');
  });

  it('a card on its way opens nothing and has no tools', () => {
    let opened = 0;
    const host = mount(() => <StuffCard {...base} pending thumb={{ kind: 'pending', icon: 'file' }} more={{ label: 'More', onClick: () => {} }} onOpen={() => opened++} />);
    host.querySelector<HTMLElement>("[data-kit='st-card']")?.click();
    expect(opened).toBe(0);
    expect(host.querySelector("[data-kit='st-tools']")).toBeNull();
    expect(host.querySelector("[data-kit='st-saving']")).not.toBeNull();
  });

  it("an app shows its window with its initial and Launch, which doesn't open the card", () => {
    const did: string[] = [];
    const host = mount(() => (
      <StuffCard {...base} kind="app" icon="app" thumb={{ kind: 'app', name: 'Hello pane', initial: 'H', excerpt: 'counts clicks' }} launch={{ label: 'Launch', title: 'Launch it in wisp', onClick: () => did.push('launch') }} onOpen={() => did.push('open')} />
    ));
    expect(host.querySelector("[data-kit='st-win-body'] b")?.textContent).toBe('H');
    host.querySelector<HTMLButtonElement>("[data-kit='st-launch']")?.click();
    expect(did).toEqual(['launch']);
  });
});

describe('the shelf', () => {
  it('a kind chip has its icon and count; a group its label and count; nothing matches offers Show everything', () => {
    let cleared = 0;
    const host = mount(() => (
      <>
        <StuffKind label="Pictures" count={1} icon="image" on onPick={() => {}} />
        <StuffGroup label="Today" count={6} />
        <StuffNone text="Nothing in My Stuff matches “zzzz”." clear="Show everything" onClear={() => cleared++} />
      </>
    ));
    expect(host.querySelector("[data-kit='sf-kind']")?.getAttribute('aria-selected')).toBe('true');
    expect(host.querySelector("[data-kit='sf-kind'] i")?.textContent).toBe('1');
    expect(host.querySelector("[data-kit='sf-head']")?.textContent).toBe('Today6');
    host.querySelector<HTMLButtonElement>("[data-kit='sf-none-box'] button")?.click();
    expect(cleared).toBe(1);
  });
});

describe('the viewer a saved picture opens in', () => {
  it('has v1s zoom bar: −, the zoom as a %, + (parity audit #9)', () => {
    mount(() => <MediaViewer list={[{ src: '/a.png', alt: 'sunset' }]} at={0} onClose={() => {}} />);
    const pct = document.querySelector<HTMLButtonElement>("[data-part='block.viewer.pct']");
    expect(pct?.textContent).toBe('100%');
    document.querySelector<HTMLButtonElement>("[data-part='block.viewer.in']")?.click();
    expect(pct?.textContent).toBe('160%');
    document.querySelector<HTMLButtonElement>("[data-part='block.viewer.out']")?.click();
    expect(pct?.textContent).toBe('100%');
  });
});
