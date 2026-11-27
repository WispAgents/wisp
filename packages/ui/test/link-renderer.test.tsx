// ADR-0145: a screen's LinkRenderer draws the links it knows (a PR's live chip) in a Markdown answer and in a
// prompt's plain text; the ones it doesn't, and every link with no renderer around, stay exactly as they were.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import type { JSX } from 'solid-js';
import { LinkedText, LinkRenderer, Markdown, textRuns, type RenderLink } from '../src/content/rich.ts';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

function mount(ui: () => JSX.Element): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(ui, host);
  return host;
}

const PR = 'https://github.com/acme/api/pull/12';
const chips: RenderLink = (href, fallback) => (href.includes('/pull/') ? <b data-chip={href}>chip</b> : fallback());

test('a prompt splits into text and its bare URLs, sentence punctuation left out', () => {
  expect(textRuns(`see ${PR}, and (https://x.dev/a).`)).toEqual([
    { v: 'see ' },
    { url: true, v: PR },
    { v: ', and (' },
    { url: true, v: 'https://x.dev/a' },
    { v: ').' },
  ]);
  expect(textRuns('no links')).toEqual([{ v: 'no links' }]);
});

test('with a renderer, a known link in an answer is its element and any other stays the anchor', () => {
  const host = mount(() => (
    <LinkRenderer render={chips}>
      <Markdown source={`Opened [the PR](${PR}). Docs: https://x.dev/a`} />
    </LinkRenderer>
  ));
  expect(host.querySelector('[data-chip]')?.getAttribute('data-chip')).toBe(PR);
  const anchors = [...host.querySelectorAll('a')].map((a) => a.getAttribute('href'));
  expect(anchors).toEqual(['https://x.dev/a']);
});

test('with no renderer an answer and a prompt render as before', () => {
  const host = mount(() => (
    <>
      <Markdown source={`[the PR](${PR})`} />
      <p data-prompt>
        <LinkedText text={`look at ${PR}`} />
      </p>
    </>
  ));
  expect(host.querySelector('a')?.getAttribute('href')).toBe(PR);
  expect(host.querySelector('[data-prompt]')?.innerHTML).toBe(`look at ${PR}`);
});

test("a prompt's PR link becomes the chip, its other text and links stay text", () => {
  const host = mount(() => (
    <LinkRenderer render={chips}>
      <p data-prompt>
        <LinkedText text={`fix ${PR} like https://x.dev/a says`} />
      </p>
    </LinkRenderer>
  ));
  const p = host.querySelector('[data-prompt]');
  expect(p?.querySelector('[data-chip]')?.getAttribute('data-chip')).toBe(PR);
  expect(p?.textContent).toBe('fix chip like https://x.dev/a says');
  expect(p?.querySelector('a')).toBeNull();
});
