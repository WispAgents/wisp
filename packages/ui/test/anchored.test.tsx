// The anchored card (v1 ui/popover.js, confirm.js ask()): placed by what opened it, kept in the window; Escape or a
// mousedown elsewhere closes it, Escape puts focus back; the ask's action has the focus.
import { createSignal, Show } from 'solid-js';
import { describe, expect, it } from 'vitest';
import { AnchoredCardImpl, AskCardImpl, placeCard } from '../src/controls/anchored-card.tsx';
import { mount } from './helpers.tsx';

const rect = (left: number, top: number, w: number, h: number): DOMRectReadOnly => ({ left, top, right: left + w, bottom: top + h, width: w, height: h, x: left, y: top, toJSON: () => ({}) }) as DOMRectReadOnly;

describe('placeCard', () => {
  it('under its anchor, right edges together (v1 ask), kept 8 px inside the window', () => {
    expect(placeCard({ w: 340, h: 160 }, rect(900, 100, 28, 28), 1280, 860, 'end')).toEqual({ x: 588, y: 134 });
    expect(placeCard({ w: 340, h: 160 }, rect(100, 100, 28, 28), 1280, 860, 'end')).toEqual({ x: 8, y: 134 });
  });
  it('above when there is no room below; Edit details lines up with the left edge; no anchor: near the top', () => {
    expect(placeCard({ w: 320, h: 300 }, rect(40, 700, 28, 28), 1280, 860, 'start')).toEqual({ x: 40, y: 394 });
    expect(placeCard({ w: 340, h: 160 }, undefined, 1280, 860, 'end')).toEqual({ x: 470, y: 155 });
  });
});

describe('AnchoredCard', () => {
  it('Escape closes it and puts focus back on its anchor; a mousedown elsewhere closes it', async () => {
    const anchor = document.createElement('button');
    document.body.append(anchor);
    const [open, setOpen] = createSignal(true);
    let picked = '';
    mount(() => (
      <Show when={open()}>
        <AnchoredCardImpl anchor={anchor} label="Allow" role="alertdialog" onClose={() => setOpen(false)}>
          <AskCardImpl question="Pin Hello pane on home? It asks to:" more={['• See your tasks', 'wisp keeps it on its own address']} actions={[{ label: 'Cancel', onClick: () => (picked = 'no') }, { label: 'Allow and pin', primary: true, onClick: () => (picked = 'ok') }]} />
        </AnchoredCardImpl>
      </Show>
    ));
    const card = document.querySelector<HTMLElement>("[data-kit='anchored']");
    expect(card?.getAttribute('role')).toBe('alertdialog');
    expect(document.activeElement?.textContent).toBe('Allow and pin');
    expect(document.querySelector("[data-kit='ask-more']")?.textContent).toBe('• See your tasks\nwisp keeps it on its own address');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(open()).toBe(false);
    expect(document.activeElement).toBe(anchor);
    setOpen(true);
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    expect(open()).toBe(false);
    expect(picked).toBe('');
  });
});
