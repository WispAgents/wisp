// The first screen's icons come with the page; the rest of the set loads on first use (ADR-0105) and still draws.
import { describe, expect, test } from 'vitest';
import { mount, part, until } from './helpers.tsx';
import { Icon } from '../src/index.ts';
import { ICONS, ICON_NAMES } from '../src/content/icons.ts';
import { FIRST_ICONS } from '../src/content/icons-first.ts';

const symbol = (name: string): Element | null => document.querySelector(`#k-icons symbol#k-i-${name}`);

describe('Icon: the first-screen set and the rest', () => {
  test('the full set is the first set plus the rest, every one on the 24 grid the outer box assumes', () => {
    for (const name of Object.keys(FIRST_ICONS)) expect(ICON_NAMES).toContain(name);
    for (const [, [box]] of Object.entries(ICONS)) expect(box).toBe('0 0 24 24');
  });

  test('a first-screen icon is in the sprite at once; one outside it joins the sprite once the set loads', async () => {
    const late = ICON_NAMES.find((n) => !(n in FIRST_ICONS));
    expect(late).toBeDefined();
    mount(() => <><Icon name="check" part="a.icon" /><Icon name={late!} part="b.icon" /></>);
    expect(symbol('check')).not.toBeNull();
    expect(part('b.icon')?.querySelector('use')?.getAttribute('href')).toBe(`#k-i-${late}`);
    await until(() => symbol(late!) !== null);
    expect(symbol(late!)?.getAttribute('viewBox')).toBe('0 0 24 24');
    // one copy each, never the first set twice
    expect(document.querySelectorAll('#k-icons symbol#k-i-check')).toHaveLength(1);
    expect(document.querySelectorAll('#k-icons symbol')).toHaveLength(ICON_NAMES.length);
  });

  test('a page drawn after the set loaded gets the whole sprite at once', () => {
    const late = ICON_NAMES.find((n) => !(n in FIRST_ICONS))!;
    mount(() => <Icon name="check" />);
    expect(symbol(late)).not.toBeNull();
  });
});
